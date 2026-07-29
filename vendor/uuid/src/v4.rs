use crate::Uuid;

impl Uuid {
    /// Creates a random UUID.
    ///
    /// This uses the operating system's RNG (via `/dev/urandom`) as the source
    /// of random numbers. If you'd like to use a custom generator, don't use
    /// this method: generate random bytes using your custom generator and pass
    /// them to the [`uuid::Builder::from_random_bytes`][from_random_bytes]
    /// function instead.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use uuid::{Uuid, Version};
    /// let uuid = Uuid::new_v4();
    ///
    /// assert_eq!(Some(Version::Random), uuid.get_version());
    /// ```
    ///
    /// # References
    ///
    /// * [UUID Version 4 in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-5.4)
    ///
    /// [from_random_bytes]: struct.Builder.html#method.from_random_bytes
    pub fn new_v4() -> Uuid {
        // This is an optimized method for generating random UUIDs that just masks
        // out the bits for the version and variant and sets them both together
        Uuid::from_u128(
            crate::rng::u128() & 0xFFFFFFFFFFFF4FFFBFFFFFFFFFFFFFFF | 0x40008000000000000000,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Variant, Version};

    #[test]
    fn test_new() {
        let uuid = Uuid::new_v4();

        assert_eq!(uuid.get_version(), Some(Version::Random));
        assert_eq!(uuid.get_variant(), Variant::RFC4122);
    }

    #[test]
    fn test_get_version() {
        let uuid = Uuid::new_v4();

        assert_eq!(uuid.get_version(), Some(Version::Random));
        assert_eq!(uuid.get_version_num(), 4)
    }
}
