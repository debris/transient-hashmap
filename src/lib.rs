//! HashMap with entries living for limited period of time.

extern crate time;

use std::mem;
use std::cmp;
use std::hash::Hash;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::{Deref, DerefMut};

pub trait Timer {
	fn get_time(&self) -> i64;
}

#[derive(Default)]
pub struct StandardTimer;

impl Timer for StandardTimer {
	fn get_time(&self) -> i64 {
		time::get_time().sec
	}
}

pub struct TransientHashMap<K, V, T = StandardTimer> where T: Timer {
	backing: HashMap<K, V>,
	timestamps: RefCell<HashMap<K, i64>>,
	lifetime: u64,
	timer: T
}

impl<K, V> TransientHashMap<K, V, StandardTimer> where K: Eq + Hash {
	pub fn new(lifetime: u64) -> Self {
		TransientHashMap::new_with_timer(lifetime, Default::default())
	}
}

impl<K, V, T> TransientHashMap<K, V, T> where K: Eq + Hash, T: Timer {
	pub fn new_with_timer(lifetime: u64, t: T) -> Self {
		TransientHashMap {
			backing: HashMap::new(),
			timestamps: RefCell::new(HashMap::new()),
			lifetime: lifetime,
			timer: t
		}
	}

	pub fn insert(&mut self, key: K, value: V) -> Option<V> where K: Clone {
		self.note_used(key.clone());
		self.backing.insert(key, value)
	}

	pub fn entry(&mut self, key: K) -> Entry<K, V> where K: Clone {
		self.note_used(key.clone());
		self.backing.entry(key)
	}

	pub fn get(&self, key: &K) -> Option<&V> where K: Clone {
		self.note_used(key.clone());
		self.backing.get(key)
	}

	pub fn get_mut(&mut self, key: &K) -> Option<&mut V> where K: Clone {
		self.note_used(key.clone());
		self.backing.get_mut(key)
	}

	pub fn contains_key(&self, key: &K) -> bool where K: Clone {
		self.note_used(key.clone());
		self.backing.contains_key(key)
	}

	pub fn remaining_lifetime(&self, key: &K) -> u64 {
		let timestamps = self.timestamps.borrow();
		match timestamps.get(key) {
			None => 0,
			Some(time) => cmp::max(0, self.lifetime - (self.timer.get_time() - time) as u64)
		}
	}

	fn note_used(&self, key: K) {
		self.timestamps.borrow_mut().insert(key, self.timer.get_time());
	}

	pub fn prune(&mut self) {
		let now = self.timer.get_time();
		let timestamps = mem::replace(&mut self.timestamps, RefCell::new(HashMap::new()));
		*self.timestamps.borrow_mut() = timestamps.into_inner().into_iter()
			.filter(|entry| ((now - entry.1) as u64) < self.lifetime)
			.collect();

		let backing = mem::replace(&mut self.backing, HashMap::new());
		let timestamps = self.timestamps.borrow();
		self.backing = backing.into_iter()
			.filter(|entry| timestamps.contains_key(&entry.0))
			.collect();
	}

	pub fn direct(&self) -> &HashMap<K, V> {
		&self.backing
	}

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
	use std::cell::RefCell;
	use super::{TransientHashMap, Timer};

	struct TestTimer<'a> {
		time: &'a RefCell<i64>
	}

	impl<'a> Timer for TestTimer<'a> {
		fn get_time(&self) -> i64 {
			*self.time.borrow()
		}
	}

    #[test]
    fn it_works() {
		let time = RefCell::new(0);
		let timer = TestTimer {
			time: &time
		};
	
		let mut t_map = TransientHashMap::new_with_timer(2, timer);
		assert_eq!(t_map.remaining_lifetime(&1), 0);

		t_map.insert(1, 1);
		assert_eq!(t_map.remaining_lifetime(&1), 2);

		*time.borrow_mut() = 1;
		assert_eq!(t_map.remaining_lifetime(&1), 1);

		*time.borrow_mut() = 2;
		assert_eq!(t_map.remaining_lifetime(&1), 0);

		*time.borrow_mut() = 1;
		assert_eq!(t_map.remaining_lifetime(&1), 1);

		t_map.prune();
		assert_eq!(t_map.remaining_lifetime(&1), 1);

		*time.borrow_mut() = 2;
		assert_eq!(t_map.remaining_lifetime(&1), 0);

		t_map.prune();
		assert_eq!(t_map.remaining_lifetime(&1), 0);

		*time.borrow_mut() = 1;
		assert_eq!(t_map.remaining_lifetime(&1), 0);
    }
}
