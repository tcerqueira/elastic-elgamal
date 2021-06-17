//! Cryptographic keys for ElGamal encryption.

use rand_core::{CryptoRng, RngCore};

use crate::{group::Group, DiscreteLogTable, Encryption};

use std::{fmt, ops};

/// Secret key for ElGamal encryption and related protocols. This is a thin wrapper around
/// the [`Group`] scalar.
// TODO: zeroize?
pub struct SecretKey<G: Group>(pub(crate) G::Scalar);

impl<G: Group> fmt::Debug for SecretKey<G> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("SecretKey")
            .field("public", &PublicKey::from(self))
            .finish()
    }
}

impl<G: Group> Clone for SecretKey<G> {
    fn clone(&self) -> Self {
        SecretKey(self.0)
    }
}

impl<G: Group> SecretKey<G> {
    pub(crate) fn new(scalar: G::Scalar) -> Self {
        SecretKey(scalar)
    }

    /// Generates a random secret key.
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        SecretKey(G::generate_scalar(rng))
    }

    /// Deserializes a secret key from bytes. If bytes do not represent a valid scalar,
    /// returns `None`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != G::SCALAR_SIZE {
            return None;
        }
        G::deserialize_scalar(bytes).map(SecretKey)
    }

    /// Exposes the scalar equivalent to this key.
    pub fn expose_scalar(&self) -> &G::Scalar {
        &self.0
    }

    /// Decrypts the provided ciphertext and returns the produced group element.
    ///
    /// As the ciphertext does not include a MAC or another way to assert integrity,
    /// this operation cannot fail. If the ciphertext is not produced properly (e.g., it targets
    /// another receiver), the returned group element will be garbage.
    // FIXME: move to `Encryption`?
    pub fn decrypt_to_element(&self, encrypted: Encryption<G>) -> G::Element {
        let dh_element = encrypted.random_element * &self.0;
        encrypted.blinded_element - dh_element
    }

    /// Decrypts the provided ciphertext and returns the original encrypted value.
    ///
    /// `lookup_table` is used to find encrypted values based on the original decrypted
    /// group element. That is, it must contain all valid plaintext values. If the value
    /// is not in the table, this method will return `None`.
    // FIXME: move to `Encryption`?
    pub fn decrypt(
        &self,
        encrypted: Encryption<G>,
        lookup_table: &DiscreteLogTable<G>,
    ) -> Option<u64> {
        lookup_table.get(&self.decrypt_to_element(encrypted))
    }
}

impl<G: Group> ops::Add for SecretKey<G> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        SecretKey(self.0 + rhs.0)
    }
}

impl<G: Group> ops::AddAssign for SecretKey<G> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0 + rhs.0;
    }
}

impl<G: Group> ops::Mul<&G::Scalar> for SecretKey<G> {
    type Output = Self;

    fn mul(self, &k: &G::Scalar) -> Self {
        SecretKey(self.0 * k)
    }
}

impl<G: Group> ops::Mul<&G::Scalar> for &SecretKey<G> {
    type Output = SecretKey<G>;

    fn mul(self, &k: &G::Scalar) -> SecretKey<G> {
        SecretKey(self.0 * k)
    }
}

/// Public key for ElGamal encryption and related protocols.
///
/// # Implementation details
///
/// We store both the original bytes (which are used in zero-knowledge proofs)
/// and its decompression into a group element.
/// This increases the memory footprint, but speeds up generating / verifying proofs.
pub struct PublicKey<G: Group> {
    pub(crate) bytes: Vec<u8>,
    pub(crate) full: G::Element,
}

impl<G: Group> Clone for PublicKey<G> {
    fn clone(&self) -> Self {
        PublicKey {
            bytes: self.bytes.clone(),
            full: self.full,
        }
    }
}

impl<G: Group> fmt::Debug for PublicKey<G> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_tuple("PublicKey")
            .field(&hex::encode(&self.bytes))
            .finish()
    }
}

impl<G> PartialEq for PublicKey<G>
where
    G: Group,
{
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl<G: Group> PublicKey<G> {
    /// Deserializes a public key from bytes. If the bytes do not represent a valid [`Group`]
    /// element, returns `None`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != G::ELEMENT_SIZE {
            return None;
        }

        G::deserialize_element(bytes)
            .filter(|element| !G::is_identity(element))
            .map(|full| PublicKey {
                bytes: bytes.to_vec(),
                full,
            })
    }

    pub(crate) fn from_element(full: G::Element) -> Self {
        let mut element_bytes = Vec::with_capacity(G::ELEMENT_SIZE);
        G::serialize_element(&full, &mut element_bytes);
        PublicKey {
            full,
            bytes: element_bytes,
        }
    }

    /// Returns bytes representing the group element corresponding to this key.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the group element equivalent to this key.
    pub fn as_element(&self) -> G::Element {
        self.full
    }
}

impl<G: Group> From<&SecretKey<G>> for PublicKey<G> {
    fn from(secret_key: &SecretKey<G>) -> Self {
        let element = G::mul_generator(&secret_key.0);
        Self::from_element(element)
    }
}

impl<G: Group> ops::Add<Self> for PublicKey<G> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let element = self.full + rhs.full;
        Self::from_element(element)
    }
}

impl<G: Group> ops::Mul<&G::Scalar> for PublicKey<G> {
    type Output = Self;

    fn mul(self, k: &G::Scalar) -> Self {
        let element = self.full * k;
        Self::from_element(element)
    }
}

/// Keypair for ElGamal encryption and related protocols, consisting of a [`SecretKey`]
/// and the matching [`PublicKey`].
pub struct Keypair<G: Group> {
    secret: SecretKey<G>,
    public: PublicKey<G>,
}

impl<G: Group> fmt::Debug for Keypair<G> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Keypair")
            .field("public", &self.public)
            .finish()
    }
}

impl<G: Group> Clone for Keypair<G> {
    fn clone(&self) -> Self {
        Keypair {
            secret: self.secret.clone(),
            public: self.public.clone(),
        }
    }
}

impl<G: Group> Keypair<G> {
    /// Generates a random keypair.
    pub fn generate<R: RngCore + CryptoRng>(rng: &mut R) -> Self {
        let secret = SecretKey::generate(rng);
        Keypair {
            public: PublicKey::from(&secret),
            secret,
        }
    }

    /// Returns the public part of this keypair.
    pub fn public(&self) -> &PublicKey<G> {
        &self.public
    }

    /// Returns the secret part of this keypair.
    pub fn secret(&self) -> &SecretKey<G> {
        &self.secret
    }

    /// Returns public and secret keys comprising this keypair.
    pub fn into_tuple(self) -> (PublicKey<G>, SecretKey<G>) {
        (self.public, self.secret)
    }
}

impl<G: Group> From<SecretKey<G>> for Keypair<G> {
    fn from(secret: SecretKey<G>) -> Self {
        Self {
            public: PublicKey::from(&secret),
            secret,
        }
    }
}

impl<G: Group> ops::Mul<&G::Scalar> for Keypair<G> {
    type Output = Self;

    fn mul(self, k: &G::Scalar) -> Self {
        Keypair {
            secret: self.secret * k,
            public: self.public * k,
        }
    }
}
