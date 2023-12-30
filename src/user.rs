use crate::kv::Serializable;

#[derive(Clone, Debug)]
pub struct User {
    pub(crate) id: u16,
    pub(crate) username: [u8; 32],
    pub(crate) password: [u8; 32],
    pub(crate) role: u8,
}

impl User {
    // User fields {
    pub fn new() -> Self {
        User {
            id: 0,
            username: Default::default(),
            password: Default::default(),
            role: 0,
        }
    }

    pub fn deserialize(data: &[u8]) -> Option<User> {
        if data.len() >= 67 {
            let id = u16::from_be_bytes([data[0], data[1]]);
            let mut username = [0u8; 32];
            username.copy_from_slice(&data[2..34]);
            let mut password = [0u8; 32];
            password.copy_from_slice(&data[34..66]);
            let role = data[66];

            Some(User { id, username, password, role })
        } else {
            None // Not enough data to deserialize
        }
    }
}

impl Serializable for User {
    fn serialize(&self, buffer: &mut [u8]) -> usize {
        let mut cursor = 0;

        // Serialize the ID
        let id_bytes = self.id.to_be_bytes();
        if buffer.len() >= cursor + 2 {
            buffer[cursor..cursor + 2].copy_from_slice(&id_bytes);
            cursor += 2;
        }

        // Serialize the username
        if buffer.len() >= cursor + 32 {
            buffer[cursor..cursor + 32].copy_from_slice(&self.username);
            cursor += 32;
        }

        // Serialize the password
        if buffer.len() >= cursor + 32 {
            buffer[cursor..cursor + 32].copy_from_slice(&self.password);
            cursor += 32;
        }

        // Serialize the role
        if buffer.len() >= cursor + 1 {
            buffer[cursor] = self.role;
            cursor += 1;
        }

        cursor // Total bytes written
    }
}
