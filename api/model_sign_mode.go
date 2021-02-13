/*
 * NetHSM
 *
 * All endpoints expect exactly the specified JSON. Additional properties will cause a Bad Request Error (400). All HTTP errors contain a JSON structure with an explanation of type string. All <a href=\"https://tools.ietf.org/html/rfc4648#section-4\">base64</a> encoded values are Big Endian.
 *
 * API version: v1
 */

// Code generated by OpenAPI Generator (https://openapi-generator.tech); DO NOT EDIT.

package api

import (
	"encoding/json"
	"fmt"
)

// SignMode the model 'SignMode'
type SignMode string

// List of SignMode
const (
	SIGNMODE_PKCS1 SignMode = "PKCS1"
	SIGNMODE_PSS_MD5 SignMode = "PSS_MD5"
	SIGNMODE_PSS_SHA1 SignMode = "PSS_SHA1"
	SIGNMODE_PSS_SHA224 SignMode = "PSS_SHA224"
	SIGNMODE_PSS_SHA256 SignMode = "PSS_SHA256"
	SIGNMODE_PSS_SHA384 SignMode = "PSS_SHA384"
	SIGNMODE_PSS_SHA512 SignMode = "PSS_SHA512"
	SIGNMODE_ED25519 SignMode = "ED25519"
)

func (v *SignMode) UnmarshalJSON(src []byte) error {
	var value string
	err := json.Unmarshal(src, &value)
	if err != nil {
		return err
	}
	enumTypeValue := SignMode(value)
	for _, existing := range []SignMode{ "PKCS1", "PSS_MD5", "PSS_SHA1", "PSS_SHA224", "PSS_SHA256", "PSS_SHA384", "PSS_SHA512", "ED25519",   } {
		if existing == enumTypeValue {
			*v = enumTypeValue
			return nil
		}
	}

	return fmt.Errorf("%+v is not a valid SignMode", value)
}

// Ptr returns reference to SignMode value
func (v SignMode) Ptr() *SignMode {
	return &v
}

type NullableSignMode struct {
	value *SignMode
	isSet bool
}

func (v NullableSignMode) Get() *SignMode {
	return v.value
}

func (v *NullableSignMode) Set(val *SignMode) {
	v.value = val
	v.isSet = true
}

func (v NullableSignMode) IsSet() bool {
	return v.isSet
}

func (v *NullableSignMode) Unset() {
	v.value = nil
	v.isSet = false
}

func NewNullableSignMode(val *SignMode) *NullableSignMode {
	return &NullableSignMode{value: val, isSet: true}
}

func (v NullableSignMode) MarshalJSON() ([]byte, error) {
	return json.Marshal(v.value)
}

func (v *NullableSignMode) UnmarshalJSON(src []byte) error {
	v.isSet = true
	return json.Unmarshal(src, &v.value)
}
