import { readFile, writeFile } from "node:fs/promises";

const version = process.argv[2]?.trim();

if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("Usage: npm run version:set -- <semver>");
  process.exit(1);
}

async function updateJson(path, updater) {
  const data = JSON.parse(await readFile(path, "utf8"));
  updater(data);
  await writeFile(path, `${JSON.stringify(data, null, 2)}\n`);
}

async function updateText(path, updater) {
  const input = await readFile(path, "utf8");
  const { output, matched } = updater(input);
  if (!matched) throw new Error(`No version field found in ${path}`);
  await writeFile(path, output);
}

function replaceVersion(input, pattern) {
  let matched = false;
  const output = input.replace(pattern, (...args) => {
    matched = true;
    return `${args[1]}${version}${args[2]}`;
  });
  return { output, matched };
}

await updateJson("package.json", (data) => {
  data.version = version;
});

await updateJson("package-lock.json", (data) => {
  data.version = version;
  if (data.packages?.[""]) {
    data.packages[""].version = version;
  }
});

await updateText("src-tauri/tauri.conf.json", (input) =>
  replaceVersion(input, /("version"\s*:\s*")[^"]+(")/),
);

await updateText("src-tauri/Cargo.toml", (input) =>
  replaceVersion(input, /(^\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m),
);

await updateText("src-tauri/Cargo.lock", (input) =>
  replaceVersion(input, /(\[\[package\]\]\r?\nname = "freeclash"\r?\nversion = ")[^"]+(")/),
);

console.log(`FreeClash version set to ${version}`);
