(async () => {
  const assert = require("node:assert/strict");
  assert(process.versions.electron, "must run in Electron");
  const { app } = require("electron");
  await app.whenReady();
  const {
    Client,
    MetadataSanitizer,
  } = require("../../crates/condite-sdk-node/dist/index.cjs");
  const { contract } = await import("./contract.mjs");
  assert.equal(await contract(Client, process.argv.at(-1)), "passed");
  console.log(
    `Electron ${process.versions.electron} main-process contract passed`,
  );
  const { privacyContract } = await import("./privacy-contract.mjs");
  const fixtures = require("./fixtures/privacy.json");
  assert.equal(await privacyContract(MetadataSanitizer, fixtures), "passed");
  console.log("Electron privacy contract passed");
  app.quit();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
