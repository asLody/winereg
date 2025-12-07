use std::fmt;

#[allow(dead_code)]
pub const REG_NONE: u32 = 0;
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;
pub const REG_BINARY: u32 = 3;
pub const REG_DWORD: u32 = 4;
#[allow(dead_code)]
pub const REG_LINK: u32 = 6;
pub const REG_MULTI_SZ: u32 = 7;
pub const REG_QWORD: u32 = 11;

#[derive(Debug, Clone, PartialEq)]
pub enum RegistryValueData {
    String(String),
    ExpandString(String),
    MultiString(Vec<String>),
    Dword(u32),
    Qword(u64),
    Binary(Vec<u8>, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegistryValue {
    pub name: String,
    pub data: RegistryValueData,
}

impl RegistryValue {
    pub fn new(name: impl Into<String>, data: RegistryValueData) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    pub fn reg_type(&self) -> u32 {
        match self.data {
            RegistryValueData::String(_) => REG_SZ,
            RegistryValueData::ExpandString(_) => REG_EXPAND_SZ,
            RegistryValueData::MultiString(_) => REG_MULTI_SZ,
            RegistryValueData::Dword(_) => REG_DWORD,
            RegistryValueData::Qword(_) => REG_QWORD,
            RegistryValueData::Binary(_, ty) => ty,
        }
    }

    pub fn raw_bytes(&self) -> Vec<u8> {
        match &self.data {
            RegistryValueData::String(v) | RegistryValueData::ExpandString(v) => {
                let mut bytes = v.encode_utf16().flat_map(|c| c.to_le_bytes()).collect::<Vec<_>>();
                bytes.extend_from_slice(&[0, 0]);
                bytes
            }
            RegistryValueData::MultiString(values) => {
                let mut bytes = Vec::new();
                for part in values {
                    bytes.extend(part.encode_utf16().flat_map(|c| c.to_le_bytes()));
                    bytes.extend_from_slice(&[0, 0]);
                }
                bytes.extend_from_slice(&[0, 0]);
                bytes
            }
            RegistryValueData::Dword(v) => v.to_le_bytes().to_vec(),
            RegistryValueData::Qword(v) => v.to_le_bytes().to_vec(),
            RegistryValueData::Binary(v, _) => v.clone(),
        }
    }
}

impl fmt::Display for RegistryValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data {
            RegistryValueData::String(v) => write!(f, "string:\"{}\"", v),
            RegistryValueData::ExpandString(v) => write!(f, "expand_string:\"{}\"", v),
            RegistryValueData::MultiString(v) => write!(f, "multi_string:{:?}", v),
            RegistryValueData::Dword(v) => write!(f, "dword:{:#010x}", v),
            RegistryValueData::Qword(v) => write!(f, "qword:{:#018x}", v),
            RegistryValueData::Binary(v, t) => write!(f, "hex({}):{}", t, v.len()),
        }
    }
}

