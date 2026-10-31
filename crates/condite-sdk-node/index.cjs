const { NativeClient } = require("./condite-sdk.node");
function normalize(error) {
  try {
    const details = JSON.parse(error.message);
    return Object.assign(new Error(details.message), details);
  } catch {
    return Object.assign(error, {
      code: "input",
      status: null,
      body: null,
      request_id: null,
      stage: null,
    });
  }
}
class Client {
  #native;
  constructor(baseUrl) {
    try {
      this.#native = new NativeClient(baseUrl);
    } catch (error) {
      throw normalize(error);
    }
  }
  async #call(method, ...args) {
    try {
      return await this.#native[method](...args);
    } catch (error) {
      throw normalize(error);
    }
  }
  health() {
    return this.#call("health");
  }
  presign(request) {
    return this.#call("presign", request);
  }
  listUploads(request = {}) {
    return this.#call("listUploads", request);
  }
  confirm(id) {
    return this.#call("confirm", id);
  }
  delete(id) {
    return this.#call("delete", id);
  }
  putUpload(signed, data) {
    return this.#call("putUpload", signed, data);
  }
  upload(request, data) {
    return this.#call("upload", request, data);
  }
}
module.exports = { Client };
