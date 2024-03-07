use lwk_jade::asyncr::Stream;
use wasm_bindgen::JsValue;
use web_sys::js_sys::Uint8Array;

struct WebSerial {
    reader: web_sys::ReadableStreamDefaultReader,
    writer: web_sys::WritableStreamDefaultWriter,
}

impl Stream for WebSerial {
    async fn read(&self, buf: &mut [u8]) -> Result<usize, lwk_jade::Error> {
        let promise = self.reader.read();
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(generic)?;
        let value = web_sys::js_sys::Reflect::get(&result, &"value".into()).map_err(generic)?;
        let data = web_sys::js_sys::Uint8Array::new(&value).to_vec();
        buf.copy_from_slice(&data);
        Ok(data.len())
    }

    async fn write(&self, buf: &[u8]) -> Result<(), lwk_jade::Error> {
        let arr = Uint8Array::new_with_length(buf.len() as u32);
        arr.copy_from(&buf);
        let promise = self.writer.write_with_chunk(&arr);
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(generic)?;
        Ok(())
    }
}

fn generic(val: JsValue) -> lwk_jade::Error {
    lwk_jade::Error::Generic(format!("{:?}", val))
}
