/// Parse [`Uuid`][uuid::Uuid]s from string literals at compile time.
///
/// ## Usage
///
/// This macro transforms the string literal representation of a
/// [`Uuid`][uuid::Uuid] into the bytes representation, raising a compilation
/// error if it cannot properly be parsed.
///
/// ## Examples
///
/// Setting a global constant:
///
/// ```
/// # use uuid::{uuid, Uuid};
/// pub const SCHEMA_ATTR_CLASS: Uuid = uuid!("00000000-0000-0000-0000-ffff00000000");
/// pub const SCHEMA_ATTR_UUID: Uuid = uuid!("00000000-0000-0000-0000-ffff00000001");
/// pub const SCHEMA_ATTR_NAME: Uuid = uuid!("00000000-0000-0000-0000-ffff00000002");
/// ```
///
/// Defining a local variable:
///
/// ```
/// # use uuid::uuid;
/// let uuid = uuid!("urn:uuid:F9168C5E-CEB2-4faa-B6BF-329BF39FA1E4");
/// ```
/// Using a const variable:
/// ```
/// # use uuid::uuid;
/// const UUID_STR: &str = "12345678-1234-5678-1234-567812345678";
/// let UUID = uuid!(UUID_STR);
/// ```
///
/// [uuid::Uuid]: https://docs.rs/uuid/*/uuid/struct.Uuid.html
#[macro_export]
macro_rules! uuid {
    ($uuid:expr) => {{
        const OUTPUT: $crate::Uuid = match $crate::Uuid::try_parse($uuid) {
            $crate::__macro_support::Ok(u) => u,
            $crate::__macro_support::Err(_) => panic!("invalid UUID"),
        };
        OUTPUT
    }};
}

// SAFETY: Callers must ensure the source and target types have an identical
// layout. Used internally to project between `Uuid`, `Bytes`, and the format
// adapter wrappers, all of which are `#[repr(transparent)]` over the same data.
macro_rules! unsafe_transmute_ref(
    ($e:expr) => { unsafe { core::mem::transmute::<&_, &_>($e) } }
);

// SAFETY: Callers must ensure the source and target types have an identical
// layout.
macro_rules! unsafe_transmute(
    ($e:expr) => { unsafe { core::mem::transmute::<_, _>($e) } }
);
