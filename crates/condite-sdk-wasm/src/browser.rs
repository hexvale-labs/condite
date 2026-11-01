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
