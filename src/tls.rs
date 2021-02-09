use arc_swap::access::Access;
use std::{
  collections::HashMap,
  fs::File,
  io::{self, BufReader, ErrorKind::InvalidData},
  path::Path,
  sync::Arc,
};
use tokio_rustls::{
  rustls::{
    internal::pemfile::{certs, rsa_private_keys},
    sign::{CertifiedKey, RSASigningKey},
    Certificate, ClientHello, PrivateKey, ResolvesServerCert,
  },
  webpki::DNSName,
};

pub fn certified_key_from_acme_certificate(certificate: acme_lib::Certificate) -> Result<CertifiedKey, io::Error> {
  let certificates = vec![Certificate(certificate.certificate_der())];
  let private_key = PrivateKey(certificate.private_key_der());
  new_certified_key(certificates, &private_key)
}

pub fn load_certified_key<P1, P2>(certificate_path: P1, private_key_path: P2) -> Result<CertifiedKey, io::Error>
where
  P1: AsRef<Path>,
  P2: AsRef<Path>,
{
  let certificates = load_certs(certificate_path)?;
  let private_key = load_key(private_key_path)?;
  new_certified_key(certificates, &private_key)
}

fn new_certified_key(certificates: Vec<Certificate>, private_key: &PrivateKey) -> Result<CertifiedKey, io::Error> {
  let private_key = RSASigningKey::new(private_key).map_err(|_| io::Error::new(InvalidData, "invalid rsa key"))?;
  Ok(CertifiedKey::new(certificates, Arc::new(Box::new(private_key))))
}

fn load_certs<P>(path: P) -> io::Result<Vec<Certificate>>
where
  P: AsRef<Path>,
{
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  certs(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid cert"))
}

fn load_key<P>(path: P) -> io::Result<PrivateKey>
where
  P: AsRef<Path>,
{
  let mut keys = load_keys(path)?;
  Ok(keys.remove(0))
}

fn load_keys<P>(path: P) -> io::Result<Vec<PrivateKey>>
where
  P: AsRef<Path>,
{
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  rsa_private_keys(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid key"))
}

pub struct ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DNSName, CertifiedKey>>,
{
  certificates: A,
}

impl<A> ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DNSName, CertifiedKey>>,
{
  pub fn new(certificates: A) -> ReconfigurableCertificateResolver<A> {
    ReconfigurableCertificateResolver { certificates }
  }
}

impl<A> ResolvesServerCert for ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DNSName, CertifiedKey>> + Send + Sync,
{
  fn resolve(&self, client_hello: ClientHello) -> Option<CertifiedKey> {
    if let Some(name) = client_hello.server_name() {
      let certificates = self.certificates.load();
      certificates.get(&name.to_owned()).cloned()
    } else {
      // This kind of resolver requires SNI
      None
    }
  }
}
