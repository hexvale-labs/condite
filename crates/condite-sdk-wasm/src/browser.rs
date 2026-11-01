use condite_sdk as sdk;
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn input<T: serde::de::DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| error(sdk::Error::Input(e.to_string())))
}
fn output<T: Serialize>(value: T) -> Result<JsValue, JsValue> {
    value
        .serialize(
            &serde_wasm_bindgen::Serializer::new()
                .serialize_large_number_types_as_bigints(true)
                .serialize_maps_as_objects(true)
                .serialize_missing_as_null(true),
        )
        .map_err(|e| error(sdk::Error::Input(e.to_string())))
}
fn error(e: sdk::Error) -> JsValue {
    let result = js_sys::Error::new(&e.to_string());
    let fields = [
        (
            "stage",
            e.stage().map(JsValue::from_str).unwrap_or(JsValue::NULL),
        ),
        ("code", JsValue::from_str(e.code())),
        (
            "status",
            e.status().map(JsValue::from).unwrap_or(JsValue::NULL),
        ),
        (
            "body",
            e.body().map(JsValue::from_str).unwrap_or(JsValue::NULL),
        ),
        (
            "request_id",
            e.request_id()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
        ),
    ];
    for (key, value) in fields {
        let _ = js_sys::Reflect::set(&result, &key.into(), &value);
    }
    result.into()
}
#[wasm_bindgen]
pub struct Client {
    inner: sdk::Client,
}
#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(base: String, api_key: Option<String>) -> Result<Client, JsValue> {
        Ok(Self {
            inner: match api_key {
                Some(key) => sdk::Client::with_api_key(base, &key),
                None => sdk::Client::new(base),
            }
            .map_err(error)?,
        })
    }
    pub async fn health(&self) -> Result<JsValue, JsValue> {
        output(self.inner.health().await.map_err(error)?)
    }
    pub async fn presign(&self, request: JsValue) -> Result<JsValue, JsValue> {
        output(self.inner.presign(&input(request)?).await.map_err(error)?)
    }
    #[wasm_bindgen(js_name = listUploads)]
    pub async fn list_uploads(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let request = if request.is_undefined() {
            sdk::ListUploadsRequest::default()
        } else {
            input(request)?
        };
        output(self.inner.list_uploads(&request).await.map_err(error)?)
    }
    pub async fn confirm(&self, id: String) -> Result<JsValue, JsValue> {
        output(self.inner.confirm(&id).await.map_err(error)?)
    }
    pub async fn download(&self, id: String) -> Result<JsValue, JsValue> {
        output(self.inner.download(&id).await.map_err(error)?)
    }
    #[wasm_bindgen(js_name = updateRetention)]
    pub async fn update_retention(
        &self,
        id: String,
        retention: JsValue,
    ) -> Result<JsValue, JsValue> {
        output(
            self.inner
                .update_retention(&id, &input(retention)?)
                .await
                .map_err(error)?,
        )
    }
    #[wasm_bindgen(js_name = createShare)]
    pub async fn create_share(&self, id: String, request: JsValue) -> Result<JsValue, JsValue> {
        output(
            self.inner
                .create_share(&id, &input(request)?)
                .await
                .map_err(error)?,
        )
    }
    #[wasm_bindgen(js_name = listShares)]
    pub async fn list_shares(&self, id: String, request: JsValue) -> Result<JsValue, JsValue> {
        let request = if request.is_undefined() {
            sdk::ListUploadsRequest::default()
        } else {
            input(request)?
        };
        output(self.inner.list_shares(&id, &request).await.map_err(error)?)
    }
    #[wasm_bindgen(js_name = revokeShare)]
    pub async fn revoke_share(&self, id: String, share_id: String) -> Result<(), JsValue> {
        self.inner.revoke_share(&id, &share_id).await.map_err(error)
    }
    pub async fn delete(&self, id: String) -> Result<(), JsValue> {
        self.inner.delete(&id).await.map_err(error)
    }
    #[wasm_bindgen(js_name = putUpload)]
    pub async fn put_upload(&self, signed: JsValue, data: Vec<u8>) -> Result<(), JsValue> {
        self.inner
            .put_upload(&input(signed)?, data)
            .await
            .map_err(error)
    }
    pub async fn upload(&self, request: JsValue, data: Vec<u8>) -> Result<JsValue, JsValue> {
        output(
            self.inner
                .upload(&input(request)?, data)
                .await
                .map_err(error)?,
        )
    }
}

fn privacy_error(e: sdk::privacy::Error) -> JsValue {
    error(sdk::Error::Input(e.to_string()))
}
fn privacy_config(value: JsValue) -> Result<sdk::privacy::Config, JsValue> {
    let config = if value.is_undefined() {
        sdk::privacy::Config::default()
    } else {
        input(value)?
    };
    config.validate().map_err(privacy_error)?;
    Ok(config)
}
#[wasm_bindgen(js_name = privacyAssess)]
pub fn privacy_assess(
    size: u64,
    header: Vec<u8>,
    config: JsValue,
    media_available: bool,
) -> Result<JsValue, JsValue> {
    output(sdk::privacy::assess_with_backend(
        size,
        &header,
        &privacy_config(config)?,
        media_available,
    ))
}
#[wasm_bindgen(js_name = privacyPrepare)]
pub fn privacy_prepare(
    data: Vec<u8>,
    filename: String,
    mime: String,
    options: JsValue,
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let prepared = sdk::privacy::prepare(
        &data,
        &filename,
        &mime,
        &input(options)?,
        &privacy_config(config)?,
    )
    .map_err(privacy_error)?;
    let value = output(
        serde_json::json!({"filename":prepared.filename,"mimeType":prepared.mime_type,"report":prepared.report}),
    )?;
    js_sys::Reflect::set(
        &value,
        &"sizeBytes".into(),
        &JsValue::from(prepared.data.len() as u64),
    )?;
    js_sys::Reflect::set(
        &value,
        &"data".into(),
        &js_sys::Uint8Array::from(prepared.data.as_slice()),
    )?;
    Ok(value)
}
#[wasm_bindgen(js_name = privacyDecrypt)]
pub fn privacy_decrypt(
    data: Vec<u8>,
    passphrase: String,
    config: JsValue,
) -> Result<js_sys::Uint8Array, JsValue> {
    let secret = zeroize::Zeroizing::new(passphrase);
    let output =
        sdk::privacy::decrypt(&data, &secret, &privacy_config(config)?).map_err(privacy_error)?;
    Ok(js_sys::Uint8Array::from(output.as_slice()))
}

#[wasm_bindgen(js_name = privacyMediaPlan)]
pub fn privacy_media_plan(data: Vec<u8>) -> Result<JsValue, JsValue> {
    output(sdk::privacy::media::plan(&data).map_err(privacy_error)?)
}
#[wasm_bindgen(js_name = privacyValidateProbe)]
pub fn privacy_validate_probe(data: Vec<u8>, config: JsValue) -> Result<(), JsValue> {
    sdk::privacy::media::validate_probe(&data, &privacy_config(config)?).map_err(privacy_error)
}
