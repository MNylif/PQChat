use std::{
	cmp::{self, Ordering},
	collections::{BTreeMap, BTreeSet, HashMap, HashSet},
	time::Duration,
};

use axum::extract::State;
use conduwuit::{
	debug, error, extract_variant, trace,
	utils::{
		math::{ruma_from_usize, usize_from_ruma},
		BoolExt, IterStream, ReadyExt, TryFutureExtExt,
	},
	warn, Error, Result,
};
use futures::{FutureExt, StreamExt, TryFutureExt};
use ruma::{
	api::client::{
		error::ErrorKind,
		sync::sync_events::{self, DeviceLists, UnreadNotificationsCount},
	},
	events::{
		room::member::{MembershipState, RoomMemberEventContent},
		AnyRawAccountDataEvent, AnySyncEphemeralRoomEvent, StateEventType, TimelineEventType,
	},
	serde::Raw,
	state_res::TypeStateKey,
	uint, DeviceId, OwnedEventId, OwnedRoomId, OwnedUserId, RoomId, UInt, UserId,
};
use service::{rooms::read_receipt::pack_receipts, PduCount};

use super::{filter_rooms, share_encrypted_room};
use crate::{
	client::{ignored_filter, sync::load_timeline, DEFAULT_BUMP_TYPES},
	Ruma,
};

type SyncInfo<'a> = (&'a UserId, &'a DeviceId, u64, &'a sync_events::v5::Request);

/// `POST /_matrix/client/unstable/org.matrix.simplified_msc3575/sync`
/// ([MSC4186])
///
/// A simplified version of sliding sync ([MSC3575]).
///
/// Get all new events in a sliding window of rooms since the last sync or a
/// given point in time.
///
/// [MSC3575]: https://github.com/matrix-org/matrix-spec-proposals/pull/3575
/// [MSC4186]: https://github.com/matrix-org/matrix-spec-proposals/pull/4186
pub(crate) async fn sync_events_v5_route(
	State(services): State<crate::State>,
	body: Ruma<sync_events::v5::Request>,
) -> Result<sync_events::v5::Response> {
	debug_assert!(DEFAULT_BUMP_TYPES.is_sorted(), "DEFAULT_BUMP_TYPES is not sorted");
	let sender_user = body.sender_user.as_ref().expect("user is authenticated");
	let sender_device = body.sender_device.as_ref().expect("user is authenticated");
	let mut body = body.body;

	// Setup watchers, so if there's no response, we can wait for them
	let watcher = services.sync.watch(sender_user, sender_device);

	let next_batch = services.globals.next_count()?;

	let conn_id = body.conn_id.clone();

	let globalsince = body
		.pos
		.as_ref()
		.and_then(|string| string.parse().ok())
		.unwrap_or(0);

	if globalsince != 0
		&& !services.sync.snake_connection_cached(
			sender_user.clone(),
			sender_device.clone(),
			conn_id.clone(),
		) {
		debug!("Restarting sync stream because it was gone from the database");
		return Err(Error::Request(
			ErrorKind::UnknownPos,
			"Connection data lost since last time".into(),
			http::StatusCode::BAD_REQUEST,
		));
	}

	// Client / User requested an initial sync
	if globalsince == 0 {
		services.sync.forget_snake_sync_connection(
			sender_user.clone(),
			sender_device.clone(),
			conn_id.clone(),
		);
	}

	// Get sticky parameters from cache
	let known_rooms = services.sync.update_snake_sync_request_with_cache(
		sender_user.clone(),
		sender_device.clone(),
		&mut body,
	);

	let all_joined_rooms: Vec<_> = services
		.rooms
		.state_cache
		.rooms_joined(sender_user)
		.map(ToOwned::to_owned)
		.collect()
		.await;

	let all_invited_rooms: Vec<_> = services
		.rooms
		.state_cache
		.rooms_invited(sender_user)
		.map(|r| r.0)
		.collect()
		.await;

	let all_knocked_rooms: Vec<_> = services
		.rooms
		.state_cache
		.rooms_knocked(sender_user)
		.map(|r| r.0)
		.collect()
		.await;

	let all_rooms: Vec<&RoomId> = all_joined_rooms
		.iter()
		.map(AsRef::as_ref)
		.chain(all_invited_rooms.iter().map(AsRef::as_ref))
		.chain(all_knocked_rooms.iter().map(AsRef::as_ref))
		.collect();

	let all_joined_rooms = all_joined_rooms.iter().map(AsRef::as_ref).collect();
	let all_invited_rooms = all_invited_rooms.iter().map(AsRef::as_ref).collect();

	let pos = next_batch.clone().to_string();

	let mut todo_rooms: TodoRooms = BTreeMap::new();

	let sync_info: SyncInfo<'_> = (sender_user, sender_device, globalsince, &body);
	let mut response = sync_events::v5::Response {
		txn_id: body.txn_id.clone(),
		pos,
		lists: BTreeMap::new(),
		rooms: BTreeMap::new(),
		extensions: sync_events::v5::response::Extensions {
			account_data: collect_account_data(services, sync_info).await,
			e2ee: collect_e2ee(services, sync_info, &all_joined_rooms).await?,
			to_device: collect_to_device(services, sync_info, next_batch).await,
			receipts: collect_receipts(services).await,
			typing: sync_events::v5::response::Typing::default(),
		},
	};

	handle_lists(
		services,
		sync_info,
		&all_invited_rooms,
		&all_joined_rooms,
		&all_rooms,
		&mut todo_rooms,
		&known_rooms,
		&mut response,
	)
	.await;

	fetch_subscriptions(services, sync_info, &known_rooms, &mut todo_rooms).await;

	response.rooms = process_rooms(
		services,
		sender_user,
		next_batch,
		&all_invited_rooms,
		&todo_rooms,
		&mut response,
		&body,
	)
	.await?;

	if response.rooms.iter().all(|(id, r)| {
		r.timeline.is_empty()
			&& r.required_state.is_empty()
			&& !response.extensions.receipts.rooms.contains_key(id)
	}) && response
		.extensions
		.to_device
		.clone()
		.is_none_or(|to| to.events.is_empty())
	{
		// Hang a few seconds so requests are not spammed
		// Stop hanging if new info arrives
		let default = Duration::from_secs(30);
		let duration = cmp::min(body.timeout.unwrap_or(default), default);
		_ = tokio::time::timeout(duration, watcher).await;
	}

	trace!(
		rooms=?response.rooms.len(),
		account_data=?response.extensions.account_data.rooms.len(),
		receipts=?response.extensions.receipts.rooms.len(),
		"responding to request with"
	);
	Ok(response)
}

type KnownRooms = BTreeMap<String, BTreeMap<OwnedRoomId, u64>>;
pub(crate) type TodoRooms = BTreeMap<OwnedRoomId, (BTreeSet<TypeStateKey>, usize, u64)>;

async fn fetch_subscriptions(
	services: crate::State,
	(sender_user, sender_device, globalsince, body): SyncInfo<'_>,
	known_rooms: &KnownRooms,
	todo_rooms: &mut TodoRooms,
) {
	let mut known_subscription_rooms = BTreeSet::new();
	for (room_id, room) in &body.room_subscriptions {
		if !services.rooms.metadata.exists(room_id).await {
			continue;
		}
		let todo_room =
			todo_rooms
				.entry(room_id.clone())
				.or_insert((BTreeSet::new(), 0_usize, u64::MAX));

		let limit: UInt = room.timeline_limit;

		todo_room.0.extend(room.required_state.iter().cloned());
		todo_room.1 = todo_room.1.max(usize_from_ruma(limit));
		// 0 means unknown because it got out of date
		todo_room.2 = todo_room.2.min(
			known_rooms
				.get("subscriptions")
				.and_then(|k| k.get(room_id))
				.copied()
				.unwrap_or(0),
		);
		known_subscription_rooms.insert(room_id.clone());
	}
	// where this went (protomsc says it was removed)
	//for r in body.unsubscribe_rooms {
	//	known_subscription_rooms.remove(&r);
	//	body.room_subscriptions.remove(&r);
	//}

	if let Some(conn_id) = &body.conn_id {
		services.sync.update_snake_sync_known_rooms(
			sender_user,
			sender_device,
			conn_id.clone(),
			"subscriptions".to_owned(),
			known_subscription_rooms,
			globalsince,
		);
	}
}

#[allow(clippy::too_many_arguments)]
async fn handle_lists<'a>(
	services: crate::State,
	(sender_user, sender_device, globalsince, body): SyncInfo<'_>,
	all_invited_rooms: &Vec<&'a RoomId>,
	all_joined_rooms: &Vec<&'a RoomId>,
	all_rooms: &Vec<&'a RoomId>,
	todo_rooms: &'a mut TodoRooms,
	known_rooms: &'a KnownRooms,
	response: &'_ mut sync_events::v5::Response,
) -> KnownRooms {
	for (list_id, list) in &body.lists {
		let active_rooms = match list.filters.clone().and_then(|f| f.is_invite) {
			| Some(true) => all_invited_rooms,
			| Some(false) => all_joined_rooms,
			| None => all_rooms,
		};

		let active_rooms = match list.filters.clone().map(|f| f.not_room_types) {
			| Some(filter) if filter.is_empty() => active_rooms,
			| Some(value) => &filter_rooms(&services, active_rooms, &value, true).await,
			| None => active_rooms,
		};

		let mut new_known_rooms: BTreeSet<OwnedRoomId> = BTreeSet::new();

		let ranges = list.ranges.clone();

		for mut range in ranges {
			range.0 = uint!(0);
			range.1 = range
				.1
				.clamp(range.0, UInt::try_from(active_rooms.len()).unwrap_or(UInt::MAX));

			let room_ids =
				active_rooms[usize_from_ruma(range.0)..usize_from_ruma(range.1)].to_vec();

			let new_rooms: BTreeSet<OwnedRoomId> =
				room_ids.clone().into_iter().map(From::from).collect();
			new_known_rooms.extend(new_rooms);
			//new_known_rooms.extend(room_ids..cloned());
			for room_id in room_ids {
				let todo_room = todo_rooms.entry(room_id.to_owned()).or_insert((
					BTreeSet::new(),
					0_usize,
					u64::MAX,
				));

				let limit: usize = usize_from_ruma(list.room_details.timeline_limit).min(100);

				todo_room
					.0
					.extend(list.room_details.required_state.iter().cloned());

				todo_room.1 = todo_room.1.max(limit);
				// 0 means unknown because it got out of date
				todo_room.2 = todo_room.2.min(
					known_rooms
						.get(list_id.as_str())
						.and_then(|k| k.get(room_id))
						.copied()
						.unwrap_or(0),
				);
			}
		}
		response
			.lists
			.insert(list_id.clone(), sync_events::v5::response::List {
				count: ruma_from_usize(active_rooms.len()),
			});

		if let Some(conn_id) = &body.conn_id {
			services.sync.update_snake_sync_known_rooms(
				sender_user,
				sender_device,
				conn_id.clone(),
				list_id.clone(),
				new_known_rooms,
				globalsince,
			);
		}
	}
	BTreeMap::default()
}

async fn process_rooms(
	services: crate::State,
	sender_user: &UserId,
	next_batch: u64,
	all_invited_rooms: &[&RoomId],
	todo_rooms: &TodoRooms,
	response: &mut sync_events::v5::Response,
	body: &sync_events::v5::Request,
) -> Result<BTreeMap<OwnedRoomId, sync_events::v5::response::Room>> {
	let mut rooms = BTreeMap::new();
	for (room_id, (required_state_request, timeline_limit, roomsince)) in todo_rooms {
		let roomsincecount = PduCount::Normal(*roomsince);

		let mut timestamp: Option<_> = None;
		let mut invite_state = None;
		let (timeline_pdus, limited);
		let new_room_id: &RoomId = (*room_id).as_ref();
		if all_invited_rooms.contains(&new_room_id) {
			// TODO: figure out a timestamp we can use for remote invites
			invite_state = services
				.rooms
				.state_cache
				.invite_state(sender_user, room_id)
				.await
				.ok();

			(timeline_pdus, limited) = (Vec::new(), true);
		} else {
			(timeline_pdus, limited) = match load_timeline(
				&services,
				sender_user,
				room_id,
				roomsincecount,
				Some(PduCount::from(next_batch)),
				*timeline_limit,
			)
			.await
			{
				| Ok(value) => value,
				| Err(err) => {
					warn!("Encountered missing timeline in {}, error {}", room_id, err);
					continue;
				},
			};
		}

		if body.extensions.account_data.enabled == Some(true) {
			response.extensions.account_data.rooms.insert(
				room_id.to_owned(),
				services
					.account_data
					.changes_since(Some(room_id), sender_user, *roomsince, Some(next_batch))
					.ready_filter_map(|e| extract_variant!(e, AnyRawAccountDataEvent::Room))
					.collect()
					.await,
			);
		}

		let last_privateread_update = services
			.rooms
			.read_receipt
			.last_privateread_update(sender_user, room_id)
			.await > *roomsince;

		let private_read_event = if last_privateread_update {
			services
				.rooms
				.read_receipt
				.private_read_get(room_id, sender_user)
				.await
				.ok()
		} else {
			None
		};

		let mut receipts: Vec<Raw<AnySyncEphemeralRoomEvent>> = services
			.rooms
			.read_receipt
			.readreceipts_since(room_id, *roomsince)
			.filter_map(|(read_user, _ts, v)| async move {
				services
					.users
					.user_is_ignored(read_user, sender_user)
					.await
					.or_some(v)
			})
			.collect()
			.await;

		if let Some(private_read_event) = private_read_event {
			receipts.push(private_read_event);
		}

		let receipt_size = receipts.len();

		if receipt_size > 0 {
			response
				.extensions
				.receipts
				.rooms
				.insert(room_id.clone(), pack_receipts(Box::new(receipts.into_iter())));
		}

		if roomsince != &0
			&& timeline_pdus.is_empty()
			&& response
				.extensions
				.account_data
				.rooms
				.get(room_id)
				.is_none_or(Vec::is_empty)
			&& receipt_size == 0
		{
			continue;
		}

		let prev_batch = timeline_pdus
			.first()
			.map_or(Ok::<_, Error>(None), |(pdu_count, _)| {
				Ok(Some(match pdu_count {
					| PduCount::Backfilled(_) => {
						error!("timeline in backfill state?!");
						"0".to_owned()
					},
					| PduCount::Normal(c) => c.to_string(),
				}))
			})?
			.or_else(|| {
				if roomsince != &0 {
					Some(roomsince.to_string())
				} else {
					None
				}
			});

		let room_events: Vec<_> = timeline_pdus
			.iter()
			.stream()
			.filter_map(|item| ignored_filter(&services, item.clone(), sender_user))
			.map(|(_, pdu)| pdu.to_sync_room_event())
			.collect()
			.await;

		for (_, pdu) in timeline_pdus {
			let ts = pdu.origin_server_ts;
			if DEFAULT_BUMP_TYPES.binary_search(&pdu.kind).is_ok()
				&& timestamp.is_none_or(|time| time <= ts)
			{
				timestamp = Some(ts);
			}
		}

		let required_state = required_state_request
			.iter()
			.stream()
			.filter_map(|state| async move {
				services
					.rooms
					.state_accessor
					.room_state_get(room_id, &state.0, &state.1)
					.await
					.map(|s| s.to_sync_state_event())
					.ok()
			})
			.collect()
			.await;

		// Heroes
		let heroes: Vec<_> = services
			.rooms
			.state_cache
			.room_members(room_id)
			.ready_filter(|member| *member != sender_user)
			.filter_map(|user_id| {
				services
					.rooms
					.state_accessor
					.get_member(room_id, user_id)
					.map_ok(|memberevent| sync_events::v5::response::Hero {
						user_id: user_id.into(),
						name: memberevent.displayname,
						avatar: memberevent.avatar_url,
					})
					.ok()
			})
			.take(5)
			.collect()
			.await;

		let name = match heroes.len().cmp(&(1_usize)) {
			| Ordering::Greater => {
				let firsts = heroes[1..]
					.iter()
					.map(|h| h.name.clone().unwrap_or_else(|| h.user_id.to_string()))
					.collect::<Vec<_>>()
					.join(", ");

				let last = heroes[0]
					.name
					.clone()
					.unwrap_or_else(|| heroes[0].user_id.to_string());

				Some(format!("{firsts} and {last}"))
			},
			| Ordering::Equal => Some(
				heroes[0]
					.name
					.clone()
					.unwrap_or_else(|| heroes[0].user_id.to_string()),
			),
			| Ordering::Less => None,
		};

		let heroes_avatar = if heroes.len() == 1 {
			heroes[0].avatar.clone()
		} else {
			None
		};

		rooms.insert(room_id.clone(), sync_events::v5::response::Room {
			name: services
				.rooms
				.state_accessor
				.get_name(room_id)
				.await
				.ok()
				.or(name),
			avatar: if let Some(heroes_avatar) = heroes_avatar {
				ruma::JsOption::Some(heroes_avatar)
			} else {
				match services.rooms.state_accessor.get_avatar(room_id).await {
					| ruma::JsOption::Some(avatar) => ruma::JsOption::from_option(avatar.url),
					| ruma::JsOption::Null => ruma::JsOption::Null,
					| ruma::JsOption::Undefined => ruma::JsOption::Undefined,
				}
			},
			initial: Some(roomsince == &0),
			is_dm: None,
			invite_state,
			unread_notifications: UnreadNotificationsCount {
				highlight_count: Some(
					services
						.rooms
						.user
						.highlight_count(sender_user, room_id)
						.await
						.try_into()
						.expect("notification count can't go that high"),
				),
				notification_count: Some(
					services
						.rooms
						.user
						.notification_count(sender_user, room_id)
						.await
						.try_into()
						.expect("notification count can't go that high"),
				),
			},
			timeline: room_events,
			required_state,
			prev_batch,
			limited,
			joined_count: Some(
				services
					.rooms
					.state_cache
					.room_joined_count(room_id)
					.await
					.unwrap_or(0)
					.try_into()
					.unwrap_or_else(|_| uint!(0)),
			),
			invited_count: Some(
				services
					.rooms
					.state_cache
					.room_invited_count(room_id)
					.await
					.unwrap_or(0)
					.try_into()
					.unwrap_or_else(|_| uint!(0)),
			),
			num_live: None, // Count events in timeline greater than global sync counter
			bump_stamp: timestamp,
			heroes: Some(heroes),
		});
	}
	Ok(rooms)
}
async fn collect_account_data(
	services: crate::State,
	(sender_user, _, globalsince, body): (&UserId, &DeviceId, u64, &sync_events::v5::Request),
) -> sync_events::v5::response::AccountData {
	let mut account_data = sync_events::v5::response::AccountData {
		global: Vec::new(),
		rooms: BTreeMap::new(),
	};

	if !body.extensions.account_data.enabled.unwrap_or(false) {
		return sync_events::v5::response::AccountData::default();
	}

	account_data.global = services
		.account_data
		.changes_since(None, sender_user, globalsince, None)
		.ready_filter_map(|e| extract_variant!(e, AnyRawAccountDataEvent::Global))
		.collect()
		.await;

	if let Some(rooms) = &body.extensions.account_data.rooms {
		for room in rooms {
			account_data.rooms.insert(
				room.clone(),
				services
					.account_data
					.changes_since(Some(room), sender_user, globalsince, None)
					.ready_filter_map(|e| extract_variant!(e, AnyRawAccountDataEvent::Room))
					.collect()
					.await,
			);
		}
	}

	account_data
}

async fn collect_e2ee<'a>(
	services: crate::State,
	(sender_user, sender_device, globalsince, body): (
		&UserId,
		&DeviceId,
		u64,
		&sync_events::v5::Request,
	),
	all_joined_rooms: &'a Vec<&'a RoomId>,
) -> Result<sync_events::v5::response::E2EE> {
	if !body.extensions.e2ee.enabled.unwrap_or(false) {
		return Ok(sync_events::v5::response::E2EE::default());
	}
	let mut left_encrypted_users = HashSet::new(); // Users that have left any encrypted rooms the sender was in
	let mut device_list_changes = HashSet::new();
	let mut device_list_left = HashSet::new();
	// Look for device list updates of this account
	device_list_changes.extend(
		services
			.users
			.keys_changed(sender_user, globalsince, None)
			.map(ToOwned::to_owned)
			.collect::<Vec<_>>()
			.await,
	);

	for room_id in all_joined_rooms {
		let Ok(current_shortstatehash) =
			services.rooms.state.get_room_shortstatehash(room_id).await
		else {
			error!("Room {room_id} has no state");
			continue;
		};

		let since_shortstatehash = services
			.rooms
			.user
			.get_token_shortstatehash(room_id, globalsince)
			.await
			.ok();

		let encrypted_room = services
			.rooms
			.state_accessor
			.state_get(current_shortstatehash, &StateEventType::RoomEncryption, "")
			.await
			.is_ok();

		if let Some(since_shortstatehash) = since_shortstatehash {
			// Skip if there are only timeline changes
			if since_shortstatehash == current_shortstatehash {
				continue;
			}

			let since_encryption = services
				.rooms
				.state_accessor
				.state_get(since_shortstatehash, &StateEventType::RoomEncryption, "")
				.await;

			let since_sender_member: Option<RoomMemberEventContent> = services
				.rooms
				.state_accessor
				.state_get_content(
					since_shortstatehash,
					&StateEventType::RoomMember,
					sender_user.as_str(),
				)
				.ok()
				.await;

			let joined_since_last_sync = since_sender_member
				.as_ref()
				.is_none_or(|member| member.membership != MembershipState::Join);

			let new_encrypted_room = encrypted_room && since_encryption.is_err();

			if encrypted_room {
				let current_state_ids: HashMap<_, OwnedEventId> = services
					.rooms
					.state_accessor
					.state_full_ids(current_shortstatehash)
					.collect()
					.await;

				let since_state_ids: HashMap<_, _> = services
					.rooms
					.state_accessor
					.state_full_ids(since_shortstatehash)
					.collect()
					.await;

				for (key, id) in current_state_ids {
					if since_state_ids.get(&key) != Some(&id) {
						let Ok(pdu) = services.rooms.timeline.get_pdu(&id).await else {
							error!("Pdu in state not found: {id}");
							continue;
						};
						if pdu.kind == TimelineEventType::RoomMember {
							if let Some(state_key) = &pdu.state_key {
								let user_id =
									OwnedUserId::parse(state_key.clone()).map_err(|_| {
										Error::bad_database("Invalid UserId in member PDU.")
									})?;

								if user_id == *sender_user {
									continue;
								}

								let content: RoomMemberEventContent = pdu.get_content()?;
								match content.membership {
									| MembershipState::Join => {
										// A new user joined an encrypted room
										if !share_encrypted_room(
											&services,
											sender_user,
											&user_id,
											Some(room_id),
										)
										.await
										{
											device_list_changes.insert(user_id);
										}
									},
									| MembershipState::Leave => {
										// Write down users that have left encrypted rooms we
										// are in
										left_encrypted_users.insert(user_id);
									},
									| _ => {},
								}
							}
						}
					}
				}
				if joined_since_last_sync || new_encrypted_room {
					// If the user is in a new encrypted room, give them all joined users
					device_list_changes.extend(
						services
						.rooms
						.state_cache
						.room_members(room_id)
						// Don't send key updates from the sender to the sender
						.ready_filter(|user_id| sender_user != *user_id)
						// Only send keys if the sender doesn't share an encrypted room with the target
						// already
						.filter_map(|user_id| {
							share_encrypted_room(&services, sender_user, user_id, Some(room_id))
								.map(|res| res.or_some(user_id.to_owned()))
						})
						.collect::<Vec<_>>()
						.await,
					);
				}
			}
		}
		// Look for device list updates in this room
		device_list_changes.extend(
			services
				.users
				.room_keys_changed(room_id, globalsince, None)
				.map(|(user_id, _)| user_id)
				.map(ToOwned::to_owned)
				.collect::<Vec<_>>()
				.await,
		);
	}

	for user_id in left_encrypted_users {
		let dont_share_encrypted_room =
			!share_encrypted_room(&services, sender_user, &user_id, None).await;

		// If the user doesn't share an encrypted room with the target anymore, we need
		// to tell them
		if dont_share_encrypted_room {
			device_list_left.insert(user_id);
		}
	}

	Ok(sync_events::v5::response::E2EE {
		device_lists: DeviceLists {
			changed: device_list_changes.into_iter().collect(),
			left: device_list_left.into_iter().collect(),
		},
		device_one_time_keys_count: services
			.users
			.count_one_time_keys(sender_user, sender_device)
			.await,
		device_unused_fallback_key_types: None,
	})
}

async fn collect_to_device(
	services: crate::State,
	(sender_user, sender_device, globalsince, body): SyncInfo<'_>,
	next_batch: u64,
) -> Option<sync_events::v5::response::ToDevice> {
	if !body.extensions.to_device.enabled.unwrap_or(false) {
		return None;
	}

	services
		.users
		.remove_to_device_events(sender_user, sender_device, globalsince)
		.await;

	Some(sync_events::v5::response::ToDevice {
		next_batch: next_batch.to_string(),
		events: services
			.users
			.get_to_device_events(sender_user, sender_device, None, Some(next_batch))
			.collect()
			.await,
	})
}

async fn collect_receipts(_services: crate::State) -> sync_events::v5::response::Receipts {
	sync_events::v5::response::Receipts { rooms: BTreeMap::new() }
	// TODO: get explicitly requested read receipts
}
