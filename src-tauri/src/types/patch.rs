use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 三态 PATCH 字段：
/// - `Unset`: 请求中未提供该字段（保持数据库原值，不参与 UPDATE SET）
/// - `Null`: 请求中显式提供了 null（在 UPDATE SET 中置为 NULL）
/// - `Value(T)`: 请求中提供了具体值（在 UPDATE SET 中更新为该值）
#[derive(Debug, Clone, PartialEq, Eq, Default, specta::Type)]
pub enum PatchField<T> {
    #[default]
    Unset,
    Null,
    Value(T),
}

impl<T> PatchField<T> {
    #[inline]
    pub fn is_unset(&self) -> bool {
        matches!(self, PatchField::Unset)
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, PatchField::Null)
    }

    #[inline]
    pub fn is_value(&self) -> bool {
        matches!(self, PatchField::Value(_))
    }

    #[inline]
    pub fn value(&self) -> Option<&T> {
        match self {
            PatchField::Value(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn into_value(self) -> Option<T> {
        match self {
            PatchField::Value(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ref(&self) -> PatchField<&T> {
        match self {
            PatchField::Unset => PatchField::Unset,
            PatchField::Null => PatchField::Null,
            PatchField::Value(ref v) => PatchField::Value(v),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchField<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|opt| match opt {
            Some(val) => PatchField::Value(val),
            None => PatchField::Null,
        })
    }
}

impl<T: Serialize> Serialize for PatchField<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PatchField::Unset => serializer.serialize_none(),
            PatchField::Null => serializer.serialize_none(),
            PatchField::Value(v) => v.serialize(serializer),
        }
    }
}
