// Writes the third-party license notices shipped with the app.
// Rust crates come from cargo-about; web packages are the ones Vite actually
// puts in the bundle, so build-only tools are left out.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { build } from "vite";

const root = join(import.meta.dirname, "..");
const output = join(root, "src-tauri/resources/THIRD-PARTY-NOTICES.html");

// Preferred in this order when a package offers several, as in about.toml.
const preferred = ["MIT", "Apache-2.0", "ISC"];

const crates = JSON.parse(
  execFileSync(
    "cargo",
    [
      "about",
      "generate",
      "--format",
      "json",
      "--locked",
      "--fail",
      "--manifest-path",
      join(root, "src-tauri/Cargo.toml"),
      "--config",
      join(root, "src-tauri/about.toml"),
    ],
    { encoding: "utf8", maxBuffer: 256 * 1024 * 1024, stdio: ["ignore", "pipe", "inherit"] },
  ),
);

const special = {
  // The npm package ships only an SPDX summary; the Rust crate of the same
  // plugin carries the license text.
  "@tauri-apps/plugin-opener": () => crateLicenseText("tauri-plugin-opener", "MIT"),
  // Only Vite's own module preload polyfill is bundled, not the dependencies
  // whose licenses follow in the same file.
  vite: (dir) =>
    readFileSync(join(dir, "LICENSE.md"), "utf8").split("# Licenses of bundled dependencies")[0],
};

// cargo-about only finds license files by the `ignore` crate's file-type globs,
// which miss names such as LICENSE_MIT or license-mit, and then fills in the
// generic SPDX text with placeholders. Those entries have no source_path; take
// the crate's own file instead when there is one.
function crateLicenses() {
  const groups = new Map();
  for (const license of crates.licenses) {
    for (const { crate } of license.used_by) {
      const own = license.source_path ? null : licenseFile(dirname(crate.manifest_path), license.id);
      const text = own ? readFileSync(own, "utf8") : license.text;
      // A crate's combined license file can match several of its licenses.
      if (!groups.has(text)) {
        groups.set(text, {
          ids: new Set(),
          names: new Set(),
          text,
          generic: !license.source_path && !own,
          users: new Map(),
        });
      }
      const group = groups.get(text);
      group.ids.add(license.id);
      group.names.add(license.name);
      group.users.set(`${crate.name} ${crate.version}`, crate);
    }
  }
  return [...groups.values()].map((group) => ({
    id: [...group.ids].sort().join(", "),
    name: [...group.names].sort().join(", "),
    text: group.text,
    generic: group.generic,
    users: [...group.users.values()],
  }));
}

const rustLicenses = crateLicenses();

function crateLicenseText(name, id) {
  const license = rustLicenses.find(
    (license) =>
      license.id.split(", ").includes(id) && license.users.some((crate) => crate.name === name),
  );
  if (!license) throw new Error(`no ${id} text for crate ${name}`);
  return license.text;
}

function licenseFile(dir, id) {
  const files = readdirSync(dir)
    .filter((file) => /^(licen[cs]e|copying)/i.test(file) && !file.endsWith(".spdx"))
    .sort();
  const token = id.split("-")[0].toLowerCase();
  const file = files.find((file) => file.toLowerCase().includes(token)) ?? (files.length === 1 ? files[0] : null);
  return file ? join(dir, file) : null;
}

async function bundledPackages() {
  const result = await build({
    root,
    logLevel: "silent",
    build: { write: false },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => r.output);
  const names = new Set();
  for (const chunk of outputs) {
    if (chunk.type !== "chunk") continue;
    for (const id of chunk.moduleIds) {
      const inPackage = id.match(/node_modules\/((?:@[^/]+\/)?[^/]+)\//);
      if (inPackage) names.add(inPackage[1]);
      else if (id.startsWith("\0vite/")) names.add("vite");
      else if (id.startsWith("\0")) throw new Error(`unknown virtual module ${id}`);
    }
  }
  // Tailwind's preflight and utilities end up in the CSS, which has no module graph.
  names.add("tailwindcss");
  return [...names].sort();
}

function packageLicenseText(name, dir, license) {
  if (special[name]) return special[name](dir);
  const id = preferred.find((id) => license.includes(id)) ?? license;
  const file = licenseFile(dir, id);
  if (!file) throw new Error(`no license text in ${dir}`);
  return readFileSync(file, "utf8");
}

function repositoryUrl(repository) {
  const url = typeof repository === "string" ? repository : repository?.url;
  if (!url) return null;
  if (/^[\w.-]+\/[\w.-]+$/.test(url)) return `https://github.com/${url}`;
  return url.replace(/^git\+/, "").replace(/\.git$/, "");
}

function webPackages(names) {
  return names.map((name) => {
    const dir = join(root, "node_modules", name);
    const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    return {
      name,
      version: manifest.version,
      license: manifest.license,
      url: repositoryUrl(manifest.repository),
      text: packageLicenseText(name, dir, manifest.license),
    };
  });
}

const escape = (text) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const link = (label, url) => (url ? `<a href="${escape(url)}">${escape(label)}</a>` : escape(label));

const block = (heading, details, text) => `<section>
<h3>${heading}</h3>
${details ? `${details}\n` : ""}<pre>${escape(text.trim())}</pre>
</section>`;

function crateSections() {
  return rustLicenses
    .map((license) => ({
      ...license,
      users: license.users.toSorted(
        (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version),
      ),
    }))
    .sort(
      (a, b) =>
        a.id.localeCompare(b.id) ||
        a.users[0].name.localeCompare(b.users[0].name) ||
        a.text.localeCompare(b.text),
    )
    .map((license) => {
      const items = license.users
        .map((crate) => {
          const source = `https://crates.io/crates/${crate.name}/${crate.version}`;
          return `<li>${link(`${crate.name} ${crate.version}`, crate.repository ?? source)}${
            license.id.includes("MPL-2.0") ? ` — source code: ${link(source, source)}` : ""
          }</li>`;
        })
        .join("");
      const note = license.generic
        ? "<p>These crates include no license file in their package, so the standard text is shown.</p>\n"
        : "";
      return block(escape(`${license.name} (${license.id})`), note + `<ul>${items}</ul>`, license.text);
    })
    .join("\n");
}

const web = webPackages(await bundledPackages());

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Tailgate third-party notices</title>
<style>
:root { color-scheme: light dark; --text: #1d1d1f; --muted: #6e6e73; --line: #d2d2d7; --code: #f5f5f7; --link: #0066cc; }
@media (prefers-color-scheme: dark) { :root { --text: #f5f5f7; --muted: #a1a1a6; --line: #424245; --code: #1c1c1e; --link: #4da3ff; } }
body { margin: 0 auto; max-width: 860px; padding: 32px 16px 64px; font: 15px/1.5 -apple-system, "Segoe UI", sans-serif; color: var(--text); background: Canvas; }
h1 { font-size: 26px; margin: 0 0 8px; }
h2 { font-size: 20px; margin: 40px 0 8px; padding-top: 16px; border-top: 1px solid var(--line); }
h3 { font-size: 15px; margin: 24px 0 4px; }
p, li { color: var(--muted); }
ul { margin: 4px 0 8px; padding-left: 20px; }
a { color: var(--link); }
pre { white-space: pre-wrap; overflow-wrap: anywhere; font: 12px/1.45 ui-monospace, Menlo, Consolas, monospace; background: var(--code); padding: 12px; border-radius: 8px; }
</style>
</head>
<body>
<h1>Third-party notices</h1>
<p>Tailgate includes the software and font below. Components under the Mozilla Public License 2.0 are used unmodified; their source code is available at the links given with each.</p>

<h2>Tailgate</h2>
${block(link("Tailgate", "https://github.com/nyabi-gh/Tailgate"), null, readFileSync(join(root, "LICENSE"), "utf8"))}

<h2>Font</h2>
${block(link("Wanted Sans", "https://github.com/wanteddev/wanted-sans"), null, readFileSync(join(root, "src/assets/fonts/WantedSans-OFL.txt"), "utf8"))}

<h2>Web interface</h2>
${web
  .map((pkg) =>
    block(
      `${link(`${pkg.name} ${pkg.version}`, pkg.url)} (${escape(pkg.license)})`,
      null,
      pkg.text,
    ),
  )
  .join("\n")}

<h2>Rust crates</h2>
${crateSections()}
</body>
</html>
`;

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, html);
console.log(`wrote ${output} (${web.length} web packages, ${crates.crates.length} crates)`);
