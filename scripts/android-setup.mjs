#!/usr/bin/env node

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = "src-tauri/gen/android";
const gradle = join(root, "app/build.gradle.kts");

function patchSigning() {
  if (!existsSync(gradle)) throw new Error("app/build.gradle.kts not found – run `pnpm tauri android init` first");
  if (!existsSync(join(root, "keystore.properties"))) {
    console.log("no keystore.properties, release stays unsigned");
    return;
  }
  let src = readFileSync(gradle, "utf8");
  if (src.includes('signingConfigs.getByName("release")')) {
    console.log("already patched", gradle);
    return;
  }
  if (!src.includes("import java.util.Properties")) {
    src = `import java.util.Properties\n${src}`;
  }
  const config = `    signingConfigs {
        create("release") {
            val props = Properties().apply {
                rootProject.file("keystore.properties").inputStream().use { load(it) }
            }
            keyAlias = props.getProperty("keyAlias")
            keyPassword = props.getProperty("password")
            storePassword = props.getProperty("password")
            storeFile = rootProject.file(props.getProperty("storeFile"))
        }
    }
    buildTypes {`;
  src = src.replace(/^    buildTypes \{/m, config);
  src = src.replace(/(getByName\("release"\) \{\n)/, `$1            signingConfig = signingConfigs.getByName("release")\n`);
  writeFileSync(gradle, src);
  console.log("patched", gradle);
}

patchSigning();
