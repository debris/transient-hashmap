#![warn(missing_docs)]

//! HashMap with entries living for limited period of time.

extern crate time;

use std::mem;
use std::cmp;
use std::hash::Hash;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::{Deref, DerefMut};

type LifetimeSec = u32;

/// Time provider.
pub trait Timer {
	/// Returns current timestamp in seconds.
	fn get_time(&self) -> i64;
}

/// Standard time provider returning current time.
#[derive(Default)]
pub struct StandardTimer;
impl Timer for StandardTimer {
	fn get_time(&self) -> i64 {
		time::get_time().sec
	}
}

/// `HashMap` with entries that will be garbage collected (pruned)
/// after not being used for specified time.
///
/// Pruning does not occur automatically, make sure to call `prune` method
/// to remove old entries.
pub struct TransientHashMap<K, V, T = StandardTimer> where T: Timer {
	backing: HashMap<K, V>,
	timestamps: RefCell<HashMap<K, i64>>,
	lifetime: LifetimeSec,
	timer: T
}

impl<K, V> TransientHashMap<K, V, StandardTimer> where K: Eq + Hash + Clone {
	/// Creates new `TransientHashMap` with standard timer and specified entries lifetime.
	pub fn new(lifetime: LifetimeSec) -> Self {
		TransientHashMap::new_with_timer(lifetime, Default::default())
	}
}

impl<K, V, T> TransientHashMap<K, V, T> where K: Eq + Hash + Clone, T: Timer {
	/// Creates new `TransientHashMap` with given timer and specfied entries lifetime.
	pub fn new_with_timer(lifetime: LifetimeSec, t: T) -> Self {
		TransientHashMap {
			backing: HashMap::new(),
			timestamps: RefCell::new(HashMap::new()),
			lifetime: lifetime,
			timer: t
		}
	}

	/// Insert new entry to this map overwriting any previous entry.
	///
	/// Prolongs lifetime of `key`.
	pub fn insert(&mut self, key: K, value: V) -> Option<V> {
		self.note_used_if(true, &key);
		self.backing.insert(key, value)
	}

	/// Insert new entry to this map overwriting any previous entry.
	///
	/// Always prolongs the lifetime of `key`.
	/// TODO [ToDr] Should only prolong if new item is inserted or entry is occupied.
	pub fn entry(&mut self, key: K) -> Entry<K, V> {
		// TODO [ToDr] note used only if occupied or inserted!
		self.note_used_if(true, &key);
		self.backing.entry(key)
	}

	/// Gets reference to stored value.
	///
	/// Prolongs lifetime of `key` if is in the map.
	pub fn get(&self, key: &K) -> Option<&V> {
		let result = self.backing.get(key);
		self.note_used_if(result.is_some(), key);
		result
	}

	/// Gets mutable reference to stored value.
	///
	/// Prolongs lifetime of `key` if is in the map.
	pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
		// This will invoke `note_used` if the key exists
		self.contains_key(key);
		self.backing.get_mut(key)
	}

	/// Checks if `key` is contained.
	///
	/// Prolongs lifetime of `key` if is in the map.
	pub fn contains_key(&self, key: &K) -> bool {
		let contains = self.backing.contains_key(key);
		self.note_used_if(contains, key);
		contains
	}

	/// Returns remaining lifetime of `key` without altering it.
	pub fn remaining_lifetime(&self, key: &K) -> Option<LifetimeSec> {
		let timestamps = self.timestamps.borrow();
		timestamps.get(key).map(|time| {
				let time = self.timer.get_time() - time;
				cmp::max(0, self.lifetime as i64 - time) as LifetimeSec
		})
	}

	#[inline]
	fn note_used_if(&self, condition: bool, key: &K) {
		if condition {
			self.timestamps.borrow_mut().insert(key.clone(), self.timer.get_time());
		}
	}

	/// Clear overdue entries from the `TransientHashMap`.
	pub fn prune(&mut self) -> Vec<K> {
		let now = self.timer.get_time();

		let timestamps = mem::replace(&mut self.timestamps, RefCell::new(HashMap::new()));
		let (ok, removed) = timestamps.into_inner().into_iter()
			.partition(|entry| now - entry.1 < self.lifetime as i64);
		*self.timestamps.borrow_mut() = ok;

		removed
			.into_iter()
			.map(|entry| {
				self.backing.remove(&entry.0);
				entry.0
			})
			.collect()
	}

	/// Get a reference to backing `HashMap`.
	pub fn direct(&self) -> &HashMap<K, V> {
		&self.backing
	}

	/// Get the mutable reference to backing `HashMap`.
	pub fn direct_mut(&mut self) -> &mut HashMap<K, V> {
		&mut self.backing
	}
}

impl<K, V, T> Deref for TransientHashMap<K, V, T> where T: Timer {
	type Target = HashMap<K, V>;

	fn deref(&self) -> &Self::Target {
		&self.backing
	}
}

impl<K, V, T> DerefMut for TransientHashMap<K, V, T> where T: Timer {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.backing
	}
}

#[cfg(test)]
mod test {
	use std::cell::Cell;
	use super::{TransientHashMap, Timer};

	struct TestTimer<'a> {
		time: &'a Cell<i64>
	}

	impl<'a> Timer for TestTimer<'a> {
		fn get_time(&self) -> i64 {
			self.time.get()
		}
	}

	#[test]
	fn should_not_track_lifetime_if_key_is_not_present() {
		// given
		let time = Cell::new(0);
		let timer = TestTimer {
			time: &time
		};
		let t_map: TransientHashMap<u64, (), _> = TransientHashMap::new_with_timer(2, timer);

		// when
		t_map.contains_key(&2);

		// then
		assert_eq!(t_map.remaining_lifetime(&2), None);
	}

	#[test]
	fn should_return_correct_lifetime_when_negative() {
		// given
		let time = Cell::new(0);
		let timer = TestTimer {
			time: &time
		};
		let mut t_map = TransientHashMap::new_with_timer(2, timer);
		t_map.insert(1, 0);

		// when
		time.set(10);

		// then
		assert_eq!(t_map.remaining_lifetime(&1), Some(0));
	}

	#[test]
	fn should_return_pruned_keys() {
		// given
		let time = Cell::new(0);
		let timer = TestTimer {
			time: &time
		};

		let mut t_map = TransientHashMap::new_with_timer(2, timer);
		t_map.insert(1, 0);
		t_map.insert(2, 0);
		t_map.insert(3, 0);
		time.set(1);
		t_map.insert(4, 0);
		assert_eq!(t_map.direct().len(), 4);

		// when
		time.set(2);
		let keys = t_map.prune();

		// then
		assert_eq!(t_map.direct().len(), 1);
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&1));
		assert!(keys.contains(&2));
		assert!(keys.contains(&3));
	}

	#[test]
	fn it_works() {
		let time = Cell::new(0);
		let timer = TestTimer {
			time: &time
		};

		let mut t_map = TransientHashMap::new_with_timer(2, timer);
		assert_eq!(t_map.remaining_lifetime(&1), None);

		t_map.insert(1, 1);
		assert_eq!(t_map.remaining_lifetime(&1), Some(2));

		time.set(1);
		assert_eq!(t_map.remaining_lifetime(&1), Some(1));

		time.set(2);
		assert_eq!(t_map.remaining_lifetime(&1), Some(0));

		time.set(1);
		assert_eq!(t_map.remaining_lifetime(&1), Some(1));

		t_map.prune();
		assert_eq!(t_map.remaining_lifetime(&1), Some(1));

		time.set(2);
		assert_eq!(t_map.remaining_lifetime(&1), Some(0));

		t_map.prune();
		assert_eq!(t_map.remaining_lifetime(&1), None);

		time.set(1);
		assert_eq!(t_map.remaining_lifetime(&1), None);
	}
}
