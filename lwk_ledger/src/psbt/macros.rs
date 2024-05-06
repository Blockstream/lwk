#[rustfmt::skip]
macro_rules! impl_pset_get_pair {
    ($rv:ident.push($slf:ident.$unkeyed_name:ident as <$unkeyed_typeval:expr, _>)) => {
        if let Some(ref $unkeyed_name) = $slf.$unkeyed_name {
            $rv.push(pset::raw::Pair {
                key: pset::raw::Key {
                    type_value: $unkeyed_typeval,
                    key: vec![],
                },
                value: pset::serialize::Serialize::serialize($unkeyed_name),
            });
        }
    };
    ($rv:ident.push($unkeyed_name:ident as <$unkeyed_typeval:expr, _>)) => {
        if let Some(ref $unkeyed_name) = $unkeyed_name {
            $rv.push(pset::raw::Pair {
                key: pset::raw::Key {
                    type_value: $unkeyed_typeval,
                    key: vec![],
                },
                value: pset::serialize::Serialize::serialize($unkeyed_name),
            });
        }
    };
    ($rv:ident.push_prop($slf:ident.$unkeyed_name:ident as <$unkeyed_typeval:expr, _>)) => {
        if let Some(ref $unkeyed_name) = $slf.$unkeyed_name {
            let key = pset::raw::ProprietaryKey::from_pset_pair($unkeyed_typeval, vec![]);
            $rv.push(pset::raw::Pair {
                key: key.to_key(),
                value: pset::serialize::Serialize::serialize($unkeyed_name),
            });
        }
    };
    ($rv:ident.push_mandatory($unkeyed_name:ident as <$unkeyed_typeval:expr, _>)) => {
            $rv.push(pset::raw::Pair {
                key: pset::raw::Key {
                    type_value: $unkeyed_typeval,
                    key: vec![],
                },
                value: pset::serialize::Serialize::serialize(&$unkeyed_name),
            });
    };
    ($rv:ident.push($slf:ident.$keyed_name:ident as <$keyed_typeval:expr, $keyed_key_type:ty>)) => {
        for (key, val) in &$slf.$keyed_name {
            $rv.push(pset::raw::Pair {
                key: pset::raw::Key {
                    type_value: $keyed_typeval,
                    key: pset::serialize::Serialize::serialize(key),
                },
                value: pset::serialize::Serialize::serialize(val),
            });
        }
    };
}

macro_rules! define_le_to_array {
    ($name: ident, $type: ty, $byte_len: expr) => {
        #[inline]
        pub fn $name(val: $type) -> [u8; $byte_len] {
            debug_assert_eq!(::std::mem::size_of::<$type>(), $byte_len); // size_of isn't a constfn in 1.22
            let mut res = [0; $byte_len];
            for i in 0..$byte_len {
                res[i] = ((val >> i * 8) & 0xff) as u8;
            }
            res
        }
    };
}

define_le_to_array!(u32_to_array_le, u32, 4);
