// Copyright 2020-2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
use base64::{
    alphabet,
    engine::{self, general_purpose},
    Engine as _,
};

use base64::engine::GeneralPurpose;
use cryptoki_sys::{
    CK_C_GetMechanismInfo, CKA_ALLOWED_MECHANISMS, CKA_ALWAYS_AUTHENTICATE, CKA_ALWAYS_SENSITIVE,
    CKA_CLASS, CKA_DECRYPT, CKA_DERIVE, CKA_EC_PARAMS, CKA_EC_POINT, CKA_ENCRYPT, CKA_EXTRACTABLE,
    CKA_ID, CKA_KEY_GEN_MECHANISM, CKA_KEY_TYPE, CKA_LABEL, CKA_LOCAL, CKA_MODIFIABLE, CKA_MODULUS,
    CKA_MODULUS_BITS, CKA_NEVER_EXTRACTABLE, CKA_PRIVATE, CKA_PUBLIC_EXPONENT, CKA_SENSITIVE,
    CKA_SIGN, CKA_SIGN_RECOVER, CKA_TOKEN, CKA_UNWRAP, CKA_VERIFY, CKA_WRAP, CKA_WRAP_WITH_TRUSTED,
    CKK_EC, CKK_ECDSA, CKK_GENERIC_SECRET, CKK_RSA, CKM_AES_CBC, CK_ATTRIBUTE_TYPE, CK_KEY_TYPE,
    CK_MECHANISM_TYPE, CK_ULONG, CK_UNAVAILABLE_INFORMATION,
};
use log::debug;
use openapi::models::{key_type, private_key, public_key, KeyMechanism, KeyType, PublicKey};
use std::collections::HashMap;
use std::mem::size_of;

// these were not in the lib
const CK_CERTIFICATE_CATEGORY_UNSPECIFIED: CK_ULONG = 0x00000000;
const CK_CERTIFICATE_CATEGORY_TOKEN_USER: CK_ULONG = 0x00000001;
const CK_CERTIFICATE_CATEGORY_AUTHORITY: CK_ULONG = 0x00000002;
const CK_CERTIFICATE_CATEGORY_OTHER_ENTITY: CK_ULONG = 0x00000003;

use super::{
    attr::{self, CkRawAttrTemplate},
    CertCategory, CertInfo, EcKeyInfo, RsaKeyInfo,
};
use crate::backend::mechanism::Mechanism;

/// Object and object attribute handling logic. See the PKCS#11
/// Section 4 on objects for more details on how these attributes
/// are handled. Each object has a unique handle and
/// a well defined class (i.e. private key, certificate etc.) and
/// based on this class a well defined set of valid attributes.
/// Since there is no R/W session support these objects are created
/// from the user provisioned database.
#[derive(Clone, Copy, Debug, Hash)]
pub struct ObjectHandle(u64);

impl From<cryptoki_sys::CK_OBJECT_HANDLE> for ObjectHandle {
    fn from(src: cryptoki_sys::CK_OBJECT_HANDLE) -> Self {
        Self(src)
    }
}

impl From<usize> for ObjectHandle {
    fn from(src: usize) -> Self {
        Self(src as u64)
    }
}

impl From<u32> for ObjectHandle {
    fn from(src: u32) -> Self {
        Self(src as u64)
    }
}

impl From<ObjectHandle> for u64 {
    fn from(src: ObjectHandle) -> Self {
        src.0
    }
}

impl From<ObjectHandle> for usize {
    fn from(src: ObjectHandle) -> Self {
        src.0 as usize
    }
}

#[derive(Debug, Clone)]
pub enum Attr {
    Bytes(Vec<u8>),
    CkBbool([u8; size_of::<cryptoki_sys::CK_BBOOL>()]),
    CkByte([u8; size_of::<cryptoki_sys::CK_BYTE>()]),
    CkKeyType([u8; size_of::<cryptoki_sys::CK_KEY_TYPE>()]),
    CkCertType([u8; size_of::<cryptoki_sys::CK_CERTIFICATE_TYPE>()]),
    CkCertCategory([u8; size_of::<cryptoki_sys::CK_ULONG>()]),
    CkMechanismType([u8; size_of::<cryptoki_sys::CK_MECHANISM_TYPE>()]),
    CkObjectClass([u8; size_of::<cryptoki_sys::CK_OBJECT_CLASS>()]),
    CkUlong([u8; size_of::<cryptoki_sys::CK_ULONG>()]),
    Sensitive,
    Null,
}

impl Attr {
    const CK_TRUE: Self = Self::CkBbool([cryptoki_sys::CK_TRUE; 1]);
    const CK_FALSE: Self = Self::CkBbool([cryptoki_sys::CK_FALSE; 1]);

    pub fn len(&self) -> usize {
        match self {
            Self::CkBbool(v) => v.len(),
            Self::CkByte(v) => v.len(),
            Self::CkKeyType(v) => v.len(),
            Self::CkCertType(v) => v.len(),
            Self::CkCertCategory(v) => v.len(),
            Self::CkMechanismType(v) => v.len(),
            Self::CkObjectClass(v) => v.len(),
            Self::CkUlong(v) => v.len(),
            Self::Bytes(v) => v.len(),
            Self::Sensitive => 0,
            Self::Null => 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::CkBbool(v) => v,
            Self::CkByte(v) => v,
            Self::CkKeyType(v) => v,
            Self::CkCertType(v) => v,
            Self::CkCertCategory(v) => v,
            Self::CkMechanismType(v) => v,
            Self::CkObjectClass(v) => v,
            Self::CkUlong(v) => v,
            Self::Bytes(v) => v,

            Self::Sensitive => &[0u8; 0],
            Self::Null => &[0u8; 0],
        }
    }

    fn from_ck_byte(src: cryptoki_sys::CK_BYTE) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkByte(src.to_le_bytes())
    }

    fn from_ck_key_type(src: cryptoki_sys::CK_KEY_TYPE) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkKeyType(src.to_le_bytes())
    }

    fn from_ck_cert_type(src: cryptoki_sys::CK_CERTIFICATE_TYPE) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkCertType(src.to_le_bytes())
    }
    fn from_ck_cert_category(src: cryptoki_sys::CK_ULONG) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkCertCategory(src.to_le_bytes())
    }

    fn from_ck_mechanism_type(src: cryptoki_sys::CK_MECHANISM_TYPE) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkMechanismType(src.to_le_bytes())
    }

    fn from_ck_mechanism_type_vec(src: Vec<cryptoki_sys::CK_MECHANISM_TYPE>) -> Self {
        #[cfg(target_endian = "little")]
        Self::Bytes(src.iter().flat_map(|x| x.to_le_bytes()).collect())
    }

    fn from_ck_object_class(src: cryptoki_sys::CK_OBJECT_CLASS) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkObjectClass(src.to_le_bytes())
    }

    fn from_ck_ulong(src: cryptoki_sys::CK_ULONG) -> Self {
        #[cfg(target_endian = "little")]
        Self::CkUlong(src.to_le_bytes())
    }
}

impl PartialEq<Attr> for Attr {
    fn eq(&self, other: &Attr) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

#[derive(Clone, Debug)]
pub enum ObjectKind {
    Mechanism(Mechanism),
    Data,
    Key,
    Certificate,
}

#[derive(Debug, Clone)]
pub struct Object {
    attrs: HashMap<cryptoki_sys::CK_ATTRIBUTE_TYPE, Attr>,
    kind: ObjectKind,
    pub id: String,
}

const KEYTYPE_EC_P224: &str = "1.3.132.0.33";
const KEYTYPE_EC_P256: &str = "1.2.840.10045.3.1.7";
const KEYTYPE_EC_P384: &str = "1.3.132.0.34";
const KEYTYPE_EC_P521: &str = "1.3.132.0.35";
const KEYTYPE_CURVE25519: &str = "1.3.101.112";

fn key_type_to_asn1(key_type: KeyType) -> Result<asn1::ObjectIdentifier, Error> {
    match key_type {
        KeyType::EcP224 => asn1::ObjectIdentifier::from_string(KEYTYPE_EC_P224)
            .ok_or(Error::KeyData("parsing key".to_string())),
        KeyType::EcP256 => asn1::ObjectIdentifier::from_string(KEYTYPE_EC_P256)
            .ok_or(Error::KeyData("parsing key".to_string())),
        KeyType::EcP384 => asn1::ObjectIdentifier::from_string(KEYTYPE_EC_P384)
            .ok_or(Error::KeyData("parsing key".to_string())),
        KeyType::EcP521 => asn1::ObjectIdentifier::from_string(KEYTYPE_EC_P521)
            .ok_or(Error::KeyData("parsing key".to_string())),
        KeyType::Curve25519 => asn1::ObjectIdentifier::from_string(KEYTYPE_CURVE25519)
            .ok_or(Error::KeyData("parsing key".to_string())),
        _ => Err(Error::KeyData("key_type".to_string())),
    }
}

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub public_key: Object,
    pub private_key: Object,
}

/*
   privKey := &CryptoObject{}
   // object.Type = TokenObject
   privKey.Handle = nextObjectHandle()
   privKey.ID = keyID
   privKey.Attributes = Attributes{}
   privKey.Attributes.Set(
       &Attribute{CKA_LABEL, []byte(keyID)},
       &Attribute{CKA_CLASS, ulongToArr(CKO_PRIVATE_KEY)},
       &Attribute{CKA_ID, []byte(keyID)},
       &Attribute{CKA_SUBJECT, nil},
       &Attribute{CKA_KEY_GEN_MECHANISM, ulongToArr(CK_UNAVAILABLE_INFORMATION)},
       &Attribute{CKA_LOCAL, FalseAttr},
       &Attribute{CKA_PRIVATE, TrueAttr},
       &Attribute{CKA_MODIFIABLE, FalseAttr},
       &Attribute{CKA_TOKEN, TrueAttr},
       &Attribute{CKA_ALWAYS_AUTHENTICATE, FalseAttr},
       &Attribute{CKA_SENSITIVE, TrueAttr},
       &Attribute{CKA_ALWAYS_SENSITIVE, TrueAttr},
       &Attribute{CKA_EXTRACTABLE, FalseAttr},
       &Attribute{CKA_NEVER_EXTRACTABLE, TrueAttr},
   )
   switch key.Type {
   case api.KEYTYPE_RSA:
       data, ok := key.GetKeyOk()
       if !ok {
           return nil, NewError("token.GetObjects", "Can't parse key data", CKR_DEVICE_ERROR)
       }
       modulusB64, ok := data.GetModulusOk()
       if !ok {
           return nil, NewError("token.GetObjects", "Can't parse key modulus", CKR_DEVICE_ERROR)
       }
       pubExpB64, ok := data.GetPublicExponentOk()
       if !ok {
           return nil, NewError("token.GetObjects", "Can't parse public key exponent", CKR_DEVICE_ERROR)
       }
       modulus, err := base64.StdEncoding.DecodeString(*modulusB64)
       if err != nil {
           return nil, err
       }
       pubExp, err := base64.StdEncoding.DecodeString(*pubExpB64)
       if err != nil {
           return nil, err
       }
       privKey.Attributes.Set(
           &Attribute{CKA_KEY_TYPE, ulongToArr(CKK_RSA)},
           &Attribute{CKA_DERIVE, FalseAttr},
           &Attribute{CKA_DECRYPT, TrueAttr},
           &Attribute{CKA_SIGN, TrueAttr},
           &Attribute{CKA_SIGN_RECOVER, FalseAttr},
           &Attribute{CKA_UNWRAP, FalseAttr},
           &Attribute{CKA_WRAP_WITH_TRUSTED, FalseAttr},
           &Attribute{CKA_MODULUS, modulus},
           &Attribute{CKA_PUBLIC_EXPONENT, pubExp},
           &Attribute{CKA_MODULUS_BITS, nil},
       )
   case api.KEYTYPE_CURVE25519,
       api.KEYTYPE_EC_P224,
       api.KEYTYPE_EC_P256,
       api.KEYTYPE_EC_P384,
       api.KEYTYPE_EC_P521:
       ecPointBytes, err := base64.StdEncoding.DecodeString(key.Key.GetData())
       if err != nil {
           return nil, err
       }
       ecPointSerialized, err := asn1.Marshal(ecPointBytes)
       if err != nil {
           return nil, err
       }
       ecParams, err := utils.KeyTypeToASN1Bytes(key.Type)
       if err != nil {
           return nil, err
       }
       var keyType CK_ULONG
       if key.Type == api.KEYTYPE_CURVE25519 {
           keyType = CKK_EC_EDWARDS
       } else {
           keyType = CKK_EC
       }
       privKey.Attributes.Set(
           &Attribute{CKA_KEY_TYPE, ulongToArr(keyType)},
           &Attribute{CKA_DERIVE, TrueAttr},
           &Attribute{CKA_DECRYPT, FalseAttr},
           &Attribute{CKA_SIGN, TrueAttr},
           &Attribute{CKA_SIGN_RECOVER, FalseAttr},
           &Attribute{CKA_UNWRAP, FalseAttr},
           &Attribute{CKA_WRAP_WITH_TRUSTED, FalseAttr},
           &Attribute{CKA_EC_PARAMS, ecParams},
           &Attribute{CKA_EC_POINT, ecPointSerialized},
       )
   default:
       return nil, NewError("token.GetObjects", "Invalid algorithm", CKR_DEVICE_ERROR)
   }
   pubKey := &CryptoObject{}
   pubKey.Handle = nextObjectHandle()
   pubKey.ID = keyID
   pubKey.Attributes = Attributes{}
   for k, v := range privKey.Attributes {
       pubKey.Attributes[k] = v
   }
   pubKey.Attributes.Set(
       &Attribute{CKA_CLASS, ulongToArr(CKO_PUBLIC_KEY)},
       &Attribute{CKA_PRIVATE, FalseAttr},
       &Attribute{CKA_SENSITIVE, FalseAttr},
       &Attribute{CKA_ALWAYS_SENSITIVE, FalseAttr},
       &Attribute{CKA_EXTRACTABLE, FalseAttr},
       &Attribute{CKA_NEVER_EXTRACTABLE, FalseAttr},
       &Attribute{CKA_DECRYPT, FalseAttr},
       &Attribute{CKA_ENCRYPT, FalseAttr},
       &Attribute{CKA_SIGN, FalseAttr},
       &Attribute{CKA_VERIFY, FalseAttr},
       &Attribute{CKA_DERIVE, FalseAttr},
       &Attribute{CKA_SIGN_RECOVER, FalseAttr},
       &Attribute{CKA_UNWRAP, FalseAttr},
       &Attribute{CKA_WRAP, FalseAttr},
       &Attribute{CKA_WRAP_WITH_TRUSTED, FalseAttr},
   )
   token.AddObject(privKey)
   token.AddObject(pubKey)
*/
#[derive(Debug)]
pub enum Error {
    KeyData(String),
    Decode(base64::DecodeError),
    Asn1Write(asn1::WriteError),
    Asn1Parse(asn1::ParseError),
    UnsupportedType,
}

fn configure_rsa(
    key_data: PublicKey,
) -> Result<(CK_KEY_TYPE, HashMap<CK_ATTRIBUTE_TYPE, Attr>), Error> {
    let key_data = key_data.key;

    let modulus = key_data
        .modulus
        .ok_or(Error::KeyData("modulus".to_string()))?;
    let public_exponent = key_data
        .public_exponent
        .ok_or(Error::KeyData("public_exponent".to_string()))?;
    let modulus = general_purpose::STANDARD
        .decode(modulus.as_bytes())
        .map_err(Error::Decode)?;
    let public_exponent = general_purpose::STANDARD
        .decode(public_exponent.as_bytes())
        .map_err(Error::Decode)?;

    let mut attrs = HashMap::new();

    attrs.insert(CKA_KEY_TYPE, Attr::from_ck_key_type(cryptoki_sys::CKK_RSA));
    attrs.insert(CKA_DERIVE, Attr::CK_FALSE);
    attrs.insert(CKA_DECRYPT, Attr::CK_TRUE);
    attrs.insert(CKA_SIGN, Attr::CK_TRUE);
    attrs.insert(CKA_SIGN_RECOVER, Attr::CK_FALSE);
    attrs.insert(CKA_UNWRAP, Attr::CK_FALSE);
    attrs.insert(CKA_WRAP_WITH_TRUSTED, Attr::CK_FALSE);
    attrs.insert(CKA_MODULUS, Attr::Bytes(modulus));
    attrs.insert(CKA_PUBLIC_EXPONENT, Attr::Bytes(public_exponent));
    attrs.insert(CKA_MODULUS_BITS, Attr::Null);
    Ok((cryptoki_sys::CKK_RSA, attrs))
}

fn configure_ec(
    key_data: PublicKey,
) -> Result<(CK_KEY_TYPE, HashMap<CK_ATTRIBUTE_TYPE, Attr>), Error> {
    let ec_points = key_data
        .key
        .data
        .ok_or(Error::KeyData("data".to_string()))?;

    let ec_point_bytes = general_purpose::STANDARD
        .decode(ec_points.as_bytes())
        .map_err(Error::Decode)?;
    let ec_point_serialized = asn1::write(|w| {
        w.write_element(&asn1::SequenceWriter::new(&|w| {
            w.write_element(&ec_point_bytes.as_slice())?;
            Ok(())
        }))
    })
    .map_err(Error::Asn1Write)?;

    let key_params = key_type_to_asn1(key_data.r#type)?;
    let ec_params = asn1::write(|w| {
        w.write_element(&asn1::SequenceWriter::new(&|w| {
            w.write_element(&key_params)?;
            Ok(())
        }))
    })
    .map_err(Error::Asn1Write)?;

    let key_type = match key_data.r#type {
        KeyType::Curve25519 => cryptoki_sys::CKK_EC_EDWARDS,
        _ => cryptoki_sys::CKK_EC,
    };
    let mut attrs = HashMap::new();

    attrs.insert(CKA_KEY_TYPE, Attr::from_ck_key_type(key_type));
    attrs.insert(CKA_DERIVE, Attr::CK_TRUE);
    attrs.insert(CKA_DECRYPT, Attr::CK_FALSE);
    attrs.insert(CKA_SIGN, Attr::CK_TRUE);
    attrs.insert(CKA_SIGN_RECOVER, Attr::CK_FALSE);
    attrs.insert(CKA_UNWRAP, Attr::CK_FALSE);
    attrs.insert(CKA_WRAP_WITH_TRUSTED, Attr::CK_FALSE);
    attrs.insert(CKA_EC_PARAMS, Attr::Bytes(ec_params));
    attrs.insert(CKA_EC_POINT, Attr::Bytes(ec_point_serialized));
    Ok((key_type, attrs))
}

pub fn from_key_data(key_data: PublicKey, id: String) -> Result<Vec<Object>, Error> {
    let mut attrs = HashMap::new();
    attrs.insert(CKA_ID, Attr::Bytes(id.as_bytes().to_vec()));
    attrs.insert(
        CKA_CLASS,
        Attr::from_ck_object_class(cryptoki_sys::CKO_PRIVATE_KEY),
    );
    attrs.insert(CKA_LABEL, Attr::Bytes(id.as_bytes().to_vec()));
    attrs.insert(
        CKA_KEY_GEN_MECHANISM,
        Attr::from_ck_mechanism_type(CK_UNAVAILABLE_INFORMATION),
    );
    attrs.insert(CKA_LOCAL, Attr::CK_FALSE);
    attrs.insert(CKA_MODIFIABLE, Attr::CK_FALSE);
    attrs.insert(CKA_TOKEN, Attr::CK_TRUE);
    attrs.insert(CKA_ALWAYS_AUTHENTICATE, Attr::CK_FALSE);
    attrs.insert(CKA_SENSITIVE, Attr::CK_TRUE);
    attrs.insert(CKA_ALWAYS_SENSITIVE, Attr::CK_TRUE);
    attrs.insert(CKA_EXTRACTABLE, Attr::CK_FALSE);
    attrs.insert(CKA_NEVER_EXTRACTABLE, Attr::CK_TRUE);
    attrs.insert(CKA_PRIVATE, Attr::CK_TRUE);

    let key_data = match key_data.r#type {
        KeyType::Rsa => configure_rsa(key_data)?,
        KeyType::Curve25519
        | KeyType::EcP224
        | KeyType::EcP256
        | KeyType::EcP384
        | KeyType::EcP521 => configure_ec(key_data)?,
        _ => {
            return Err(Error::UnsupportedType);
        }
    };
    attrs.extend(key_data.1);

    let mut public_key = Object {
        attrs: attrs.clone(),
        kind: ObjectKind::Key,
        id: id.clone(),
    };

    let private_key = Object {
        attrs,
        kind: ObjectKind::Key,
        id,
    };

    public_key.attrs.insert(
        CKA_CLASS,
        Attr::from_ck_object_class(cryptoki_sys::CKO_PUBLIC_KEY),
    );
    public_key
        .attrs
        .insert(CKA_KEY_TYPE, Attr::from_ck_key_type(key_data.0));

    public_key.attrs.insert(CKA_PRIVATE, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_SENSITIVE, Attr::CK_FALSE);
    public_key
        .attrs
        .insert(CKA_ALWAYS_SENSITIVE, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_EXTRACTABLE, Attr::CK_FALSE);
    public_key
        .attrs
        .insert(CKA_NEVER_EXTRACTABLE, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_DECRYPT, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_ENCRYPT, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_SIGN, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_VERIFY, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_DERIVE, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_SIGN_RECOVER, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_UNWRAP, Attr::CK_FALSE);
    public_key.attrs.insert(CKA_WRAP, Attr::CK_FALSE);
    public_key
        .attrs
        .insert(CKA_WRAP_WITH_TRUSTED, Attr::CK_FALSE);

    Ok(vec![public_key, private_key])
}

impl Object {
    pub fn attr(&self, attr_type: cryptoki_sys::CK_ATTRIBUTE_TYPE) -> Option<&Attr> {
        self.attrs.get(&attr_type)
    }

    pub fn kind(&self) -> &ObjectKind {
        &self.kind
    }

    pub fn is_private(&self) -> bool {
        match self.attr(cryptoki_sys::CKA_PRIVATE) {
            Some(attr) => *attr == Attr::CK_TRUE,
            _ => false,
        }
    }

    pub fn is_mechanism(&self) -> bool {
        match self.kind {
            ObjectKind::Mechanism(_) => true,
            _ => false,
        }
    }

    pub fn match_attr_template(&self, tpl: &CkRawAttrTemplate) -> bool {
        let mut class_matched = false;
        for raw_attr in tpl.iter() {
            match self.attr(raw_attr.type_()) {
                Some(attr) => match raw_attr.val_bytes() {
                    Some(raw_bytes) => {
                        if attr.as_bytes() != raw_bytes {
                            return false;
                        }
                    }
                    None => return false,
                },
                None => return false,
            };
            class_matched = class_matched || (raw_attr.type_() == cryptoki_sys::CKA_CLASS);
        }

        // Per the PKCS#11 v2.40 spec, mechanism objects must only match templates that
        // explicitely provide CKA_CLASS = CKO_MECHANISM.
        if self.is_mechanism() {
            class_matched
        } else {
            true
        }
    }

    pub fn fill_attr_template(&self, tpl: &mut CkRawAttrTemplate) -> cryptoki_sys::CK_RV {
        let mut rcode = cryptoki_sys::CKR_OK;

        for mut raw_attr in tpl.iter() {
            match self.attr(raw_attr.type_()) {
                Some(attr) => {
                    let sres = match attr {
                        Attr::Sensitive => {
                            rcode = cryptoki_sys::CKR_ATTRIBUTE_SENSITIVE;
                            raw_attr.set_len(cryptoki_sys::CK_UNAVAILABLE_INFORMATION);
                            continue;
                        }
                        a => raw_attr.set_val_bytes(a.as_bytes()),
                    };
                    match sres {
                        Err(attr::Error::BufTooSmall) => {
                            rcode = cryptoki_sys::CKR_BUFFER_TOO_SMALL;
                            raw_attr.set_len(cryptoki_sys::CK_UNAVAILABLE_INFORMATION);
                        }
                        _ => raw_attr.set_len(attr.len() as cryptoki_sys::CK_ULONG),
                    };
                }
                None => {
                    rcode = cryptoki_sys::CKR_ATTRIBUTE_TYPE_INVALID;
                    raw_attr.set_len(cryptoki_sys::CK_UNAVAILABLE_INFORMATION);
                }
            };
            debug!(
                "fill_attr_template: {:?} | code : {:?}",
                raw_attr.type_(),
                rcode
            );
        }
        rcode
    }
}