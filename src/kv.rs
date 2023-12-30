pub const MAX_ITEMS: usize = 50; // Maximum number of items the database can hold

pub struct KeyValuePair<K, V> {
    key: K,
    value: V,
}

pub struct KeyValueStore<K, V> {
    items: [Option<KeyValuePair<K, V>>; MAX_ITEMS],
    count: usize,
}

pub trait Serializable {
    // Serialize the object into the provided buffer, returning the number of bytes written.
    fn serialize(&self, buffer: &mut [u8]) -> usize;
}

impl<K, V> KeyValueStore<K, V>
    where
        K: Serializable + core::cmp::PartialEq,
        V: Serializable,
{

    pub fn serialize(&self, buffer: &mut [u8]) -> usize {
        let mut cursor = 0;

        for item in self.items.iter() {
            if let Some(KeyValuePair { key, value }) = item {
                cursor += key.serialize(&mut buffer[cursor..]);
                cursor += value.serialize(&mut buffer[cursor..]);
            }
        }

        cursor // Total number of bytes written
    }

    pub fn new() -> Self {
        // Use a loop to initialize each element of the array
        let mut items = core::mem::MaybeUninit::<[Option<KeyValuePair<K, V>>; MAX_ITEMS]>::uninit();
        let items_ptr = items.as_mut_ptr();

        for i in 0..MAX_ITEMS {
            unsafe { core::ptr::write(items_ptr.cast::<Option<KeyValuePair<K, V>>>().add(i), None) };
        }

        KeyValueStore {
            items: unsafe { items.assume_init() },
            count: 0,
        }
    }

    pub fn add(&mut self, key: K, value: V) -> Result<(), &'static str> {
        if self.count >= MAX_ITEMS {
            return Err("Database is full");
        }

        for item in self.items.iter_mut() {
            if item.is_none() {
                *item = Some(KeyValuePair { key, value });
                self.count += 1;
                return Ok(());
            }
        }

        Err("Failed to add item")
    }

    pub fn set(&mut self, key: K, value: V) -> Result<(), &'static str> {
        for item in self.items.iter_mut() {
            if let Some(KeyValuePair { key: ref existing_key, value: ref mut existing_value }) = item {
                if *existing_key == key {
                    *existing_value = value;
                    return Ok(());
                }
            }
        }

        // If key is not found, try to add it
        self.add(key, value)
    }

    pub fn get(&self, key: &K) -> Option<&V>
        where
            K: PartialEq,
    {
        for item in self.items.iter() {
            if let Some(KeyValuePair { key: ref k, value: ref v }) = item {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }
}

impl Serializable for u16 {
    fn serialize(&self, buffer: &mut [u8]) -> usize {
        if buffer.len() >= 2 {
            buffer[0] = (*self >> 8) as u8; // High byte
            buffer[1] = *self as u8;       // Low byte
            2 // Number of bytes written
        } else {
            0 // Not enough space in buffer
        }
    }
}

#[derive(Clone, Debug)]
struct UserDummy {
    id: u16,
}

impl UserDummy {
    // User fields {
    fn new() -> Self {
        UserDummy {
            id: 0
        }
    }

    fn serialize(&self) -> [u8; 2] {
        self.id.to_be_bytes()
    }

    fn deserialize(data: [u8; 2]) -> UserDummy {
        let id = u16::from_be_bytes(data);
        UserDummy { id }
    }
}

impl Serializable for UserDummy {
    fn serialize(&self, buffer: &mut [u8]) -> usize {
        let bytes = self.id.to_be_bytes();
        buffer[..2].copy_from_slice(&bytes);
        2 // number of bytes written
    }
}


#[cfg(test)]
mod tests {
    use super::*; // Import your http module functions

    #[test]
    fn test_kv() {
        let mut id_store = KeyValueStore::<u16, u16>::new();
        let mut store = KeyValueStore::<u16, UserDummy>::new();

        let mut user = UserDummy::new();
        let mut user2 = UserDummy::new();

        user.id = id_store.get(&0).cloned().unwrap_or(1);
        id_store.set(0, user.id + 1).unwrap();
        user2.id = id_store.get(&0).cloned().unwrap_or(1);
        id_store.set(0, user.id + 1).unwrap();

        // Clone user and user2 before adding them to the store
        store.add(1u16, user.clone()).unwrap();
        store.add(2u16, user2.clone()).unwrap();

        if let Some(value) = store.get(&1u16) {
            assert_eq!(value.id, user.id, "Expected User.id, found {:?}", user.id);
        }

        // Serialize
        let bytes = user.serialize();
        let bytes2 = user2.serialize();
        assert_eq!(bytes, [0, 1], "Expected User.id, found {:?}", user);
        assert_eq!(bytes2, [0, 2], "Expected User.id, found {:?}", user2);

        // Deserialize
        let loaded_user = UserDummy::deserialize(bytes);
    }
}