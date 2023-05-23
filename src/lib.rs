



/// A Store of values accessible through [Handle]s.
#[derive(Debug)]
pub struct Store<T> {
	values: Vec<Slot<T>>,
	/// Tracking value for the number of allocations to enure
	/// that every [Handle] is unique.
	alloc_idx: usize,
}


impl<T> Store<T> {
	pub fn new() -> Self {
		static INSTANCE_COUNT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

		let c = INSTANCE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

		Self {
			values: Vec::new(),
			alloc_idx: usize::from_be_bytes([c, 0, 0, 0, 0, 0, 0, 0]),
		}
	}

	/// Clear the store, invalidating all [Handle]s.
	pub fn clear(&mut self) {
		self.values.clear()
	}

	/// Insert a value into the Store, returning a [Handle] to its [Slot].
	pub fn insert(&mut self, value: T) -> Handle<T> {
		let handle = Handle::new(self.values.len(), self.alloc_idx);
		self.values.push(Slot::new_occupied(value, self.alloc_idx));
		self.alloc_idx += 1;
		handle
	}

	/// Allocate a [Slot] within the Store and return a [Handle] to it.
	pub fn alloc(&mut self) -> Handle<T> {
		let handle = Handle::new(self.values.len(), self.alloc_idx);
		self.values.push(Slot::new_empty(self.alloc_idx));
		self.alloc_idx += 1;
		handle
	}

	fn check_handle(handle: Handle<T>, stored_alloc_idx: usize) -> Result<()> {
		// check if the handle is still referring to the expected value
		if stored_alloc_idx != handle.alloc_idx {

			// check if the handle was even created by this store to begin with
			if stored_alloc_idx.to_be_bytes()[0] != handle.alloc_idx.to_be_bytes()[0] {
				return Err(StoreError::WrongStore);
			}

			return Err(StoreError::StoreMutated);
		}

		Ok(())
	}

	/// Set the value at `handle` to `value`, if the given [Handle]
	/// points at something.
	/// Returns the previous [Slot].
	pub fn set(&mut self, handle: Handle<T>, value: T) -> Result<Option<T>> {
		match self.values.get_mut(handle.index) {
			Some(slot) => {
				Store::check_handle(handle, slot.alloc_idx)?;
				Ok(slot.swap(value))
			},
			None => {
				Self::check_handle(handle, self.alloc_idx)?;
				Err(StoreError::StoreMutated)
			},
		}
	}

	/// Remove the value at `handle`, if present, and return it,
	/// leaving the slot empty.
	pub fn take(&mut self, handle: Handle<T>) -> Result<T> {
		let slot = self.values.get_mut(handle.index)
			.ok_or(StoreError::StoreMutated)?;

		Self::check_handle(handle, slot.alloc_idx)?;

		slot.take().ok_or(StoreError::SlotEmpty)
	}

	/// Get a reference to the value at `handle`, if present.
	///
	/// - Returns [StoreError::SlotEmpty] if the slot was empty.
	/// - Returns [StoreError::StoreMutated] or [StoreError::WrongStore]
	/// if `handle` is invalid.
	pub fn get(&self, handle: Handle<T>) -> Result<&T> {
		match self.values.get(handle.index) {
			Some(slot) => {
				Self::check_handle(handle, slot.alloc_idx)?;
				slot.as_ref().ok_or(StoreError::SlotEmpty)
			},
			None => {
				Self::check_handle(handle, self.alloc_idx)?;
				Err(StoreError::StoreMutated)
			},
		}
	}

	/// Get a reference to the value at `handle`, evading all safety checks.
	pub unsafe fn get_unchecked(&self, handle: Handle<T>) -> Option<&T> {
		self.values.get_unchecked(handle.index).as_ref()
	}

	/// Get a mutable reference to the value at `handle`, if present.
	///
	/// - Returns [StoreError::SlotEmpty] if the slot was empty.
	/// - Returns [StoreError::StoreMutated] or [StoreError::WrongStore]
	/// if `handle` is invalid.
	pub fn get_mut(&mut self, handle: Handle<T>) -> Result<&mut T> {
		match self.values.get_mut(handle.index) {
			Some(slot) => {
				Self::check_handle(handle, slot.alloc_idx)?;
				slot.as_mut().ok_or(StoreError::SlotEmpty)
			},
			None => {
				Self::check_handle(handle, self.alloc_idx)?;
				Err(StoreError::StoreMutated)
			},
		}
	}

	/// Get a mutable reference to the value at `handle`, evading all safety checks.
	pub unsafe fn get_unchecked_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
		self.values.get_unchecked_mut(handle.index).as_mut()
	}
}

impl<T> std::ops::Index<Handle<T>> for Store<T> {
	type Output = Option<T>;

	fn index(&self, index: Handle<T>) -> &Self::Output {
		&self.values[index.index].value
	}
}

impl<T> std::ops::IndexMut<Handle<T>> for Store<T> {
	fn index_mut(&mut self, index: Handle<T>) -> &mut Self::Output {
		&mut self.values[index.index].value
	}
}


/// A Handle possibly pointing to a value within a [Store]
#[derive(Clone, Copy)]
pub struct Handle<T> {
	pub(self) index: usize,
	pub(self) alloc_idx: usize,
	_marker: std::marker::PhantomData<T>
}

impl<T> Handle<T> {
	pub(self) fn new(index: usize, alloc_idx: usize) -> Self {
		Self {
			index,
			alloc_idx,
			_marker: std::marker::PhantomData,
		}
	}
}

impl<T> std::fmt::Debug for Handle<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}({})", std::any::type_name::<Self>(), self.index)
	}
}


/// A slot of some data.
#[derive(Debug, Clone, Copy, Hash, PartialEq, PartialOrd)]
pub struct Slot<T> {
	pub(self) value: Option<T>,
	/// The allocation index to ensure [Handle] uniqueness.
	pub(self) alloc_idx: usize,
}

impl<T> Slot<T> {
	pub(self) fn new_empty(alloc_idx: usize) -> Self {
		Self {
			value: None,
			alloc_idx,
		}
	}

	pub(self) fn new_occupied(val: T, alloc_idx: usize) -> Self {
		Self {
			value: Some(val),
			alloc_idx,
		}
	}

	pub fn take(&mut self) -> Option<T> {
		std::mem::replace(&mut self.value, None)
	}

	pub fn swap(&mut self, val: T) -> Option<T> {
		std::mem::replace(&mut self.value, Some(val))
	}
}

impl<T> std::ops::Deref for Slot<T> {
	type Target = Option<T>;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}

impl<T> std::ops::DerefMut for Slot<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}


pub type Result<T> = std::result::Result<T, StoreError>;

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum StoreError {
	/// Handle was invalidated by mutating the store
	#[error("was invalidated by mutating the store")]
	StoreMutated,
	/// Handle refers to a value from another store
	#[error("handle refers to a value from another store")]
	WrongStore,

	#[error("slot was empty")]
	SlotEmpty
}



#[test]
fn test() {
	let mut store = Store::new();
	let handle = store.insert(12);

	// get, get_unchecked
	assert_eq!(store.get(handle), Ok(&12));
	assert_eq!(&store[handle], &Some(12));

	// get_mut, get_mut_unchecked
	assert_eq!(store.get_mut(handle), Ok(&mut 12));

	let mut_ref = &mut store[handle];
	assert_eq!(mut_ref, &mut Some(12));

	*mut_ref = Some(14);
	assert_eq!(store.get(handle), Ok(&14));


	// alloc
	let handle = store.alloc();
	assert_eq!(store.get(handle), Err(StoreError::SlotEmpty));

	// take
	let handle = store.insert(10);
	assert_eq!(store.take(handle), Ok(10));
	assert_eq!(store.get(handle), Err(StoreError::SlotEmpty));


	// StoreError::StoreMutated
	store.clear();
	assert_eq!(store.get(handle), Err(StoreError::StoreMutated));

	// StoreError::WrongStore
	let store = Store::new();
	assert_eq!(store.get(handle), Err(StoreError::WrongStore));


	// auto traits
	fn auto_traits<T: Send + Sync + Unpin>(_: T) {}
	auto_traits(store);
}
