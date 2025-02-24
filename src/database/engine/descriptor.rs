use conduwuit::utils::string::EMPTY;
use rocksdb::{
	DBCompactionPri as CompactionPri, DBCompactionStyle as CompactionStyle,
	DBCompressionType as CompressionType,
};

use super::cf_opts::SENTINEL_COMPRESSION_LEVEL;

#[derive(Debug, Clone, Copy)]
pub(crate) enum CacheDisp {
	Unique,
	Shared,
	SharedWith(&'static str),
}

#[derive(Debug, Clone)]
pub(crate) struct Descriptor {
	pub(crate) name: &'static str,
	pub(crate) dropped: bool,
	pub(crate) cache_disp: CacheDisp,
	pub(crate) key_size_hint: Option<usize>,
	pub(crate) val_size_hint: Option<usize>,
	pub(crate) block_size: usize,
	pub(crate) index_size: usize,
	pub(crate) write_size: usize,
	pub(crate) cache_size: usize,
	pub(crate) level_size: u64,
	pub(crate) level_shape: [i32; 7],
	pub(crate) file_size: u64,
	pub(crate) file_shape: i32,
	pub(crate) level0_width: i32,
	pub(crate) merge_width: (i32, i32),
	pub(crate) ttl: u64,
	pub(crate) compaction: CompactionStyle,
	pub(crate) compaction_pri: CompactionPri,
	pub(crate) compression: CompressionType,
	pub(crate) compressed_index: bool,
	pub(crate) compression_shape: [i32; 7],
	pub(crate) compression_level: i32,
	pub(crate) bottommost_level: Option<i32>,
	pub(crate) block_index_hashing: Option<bool>,
	pub(crate) cache_shards: u32,
	pub(crate) write_to_cache: bool,
	pub(crate) auto_readahead_thresh: u32,
	pub(crate) auto_readahead_init: usize,
	pub(crate) auto_readahead_max: usize,
}

pub(crate) static BASE: Descriptor = Descriptor {
	name: EMPTY,
	dropped: false,
	cache_disp: CacheDisp::Shared,
	key_size_hint: None,
	val_size_hint: None,
	block_size: 1024 * 4,
	index_size: 1024 * 4,
	write_size: 1024 * 1024 * 2,
	cache_size: 1024 * 1024 * 4,
	level_size: 1024 * 1024 * 8,
	level_shape: [1, 1, 1, 3, 7, 15, 31],
	file_size: 1024 * 1024,
	file_shape: 2,
	level0_width: 2,
	merge_width: (2, 16),
	ttl: 60 * 60 * 24 * 21,
	compaction: CompactionStyle::Level,
	compaction_pri: CompactionPri::MinOverlappingRatio,
	compression: CompressionType::Zstd,
	compressed_index: true,
	compression_shape: [0, 0, 0, 1, 1, 1, 1],
	compression_level: SENTINEL_COMPRESSION_LEVEL,
	bottommost_level: Some(SENTINEL_COMPRESSION_LEVEL),
	block_index_hashing: None,
	cache_shards: 64,
	write_to_cache: false,
	auto_readahead_thresh: 0,
	auto_readahead_init: 1024 * 16,
	auto_readahead_max: 1024 * 1024 * 2,
};

pub(crate) static RANDOM: Descriptor = Descriptor {
	compaction_pri: CompactionPri::OldestSmallestSeqFirst,
	write_size: 1024 * 1024 * 32,
	cache_shards: 128,
	compression_level: -3,
	bottommost_level: Some(-1),
	compressed_index: true,
	..BASE
};

pub(crate) static SEQUENTIAL: Descriptor = Descriptor {
	compaction_pri: CompactionPri::OldestLargestSeqFirst,
	write_size: 1024 * 1024 * 64,
	level_size: 1024 * 1024 * 32,
	file_size: 1024 * 1024 * 2,
	cache_shards: 128,
	compression_level: -2,
	bottommost_level: Some(-1),
	compression_shape: [0, 0, 1, 1, 1, 1, 1],
	compressed_index: false,
	..BASE
};

pub(crate) static RANDOM_SMALL: Descriptor = Descriptor {
	compaction: CompactionStyle::Universal,
	write_size: 1024 * 1024 * 16,
	level_size: 1024 * 512,
	file_size: 1024 * 128,
	file_shape: 3,
	index_size: 512,
	block_size: 512,
	cache_shards: 64,
	compression_level: -4,
	bottommost_level: Some(-1),
	compression_shape: [0, 0, 0, 0, 0, 1, 1],
	compressed_index: false,
	..RANDOM
};

pub(crate) static SEQUENTIAL_SMALL: Descriptor = Descriptor {
	compaction: CompactionStyle::Universal,
	write_size: 1024 * 1024 * 16,
	level_size: 1024 * 1024,
	file_size: 1024 * 512,
	file_shape: 3,
	block_size: 512,
	cache_shards: 64,
	block_index_hashing: Some(false),
	compression_level: -4,
	bottommost_level: Some(-2),
	compression_shape: [0, 0, 0, 0, 1, 1, 1],
	compressed_index: false,
	..SEQUENTIAL
};
