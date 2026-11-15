// Versão do Pixel Studio Maker: lê, calcula a próxima e grava em todos os arquivos que a guardam.
// Usado pelo workflow .github/workflows/versao.yml, e também à mão.
//
//   node scripts/versao.mjs atual                      → 0.1.0 (do Cargo.toml)
//   node scripts/versao.mjs proxima patch|minor|major [base]   → 0.1.1 (base padrão: a atual)
//   node scripts/versao.mjs conferir 0.2.0             → erro se algum arquivo tiver outra versão
//   node scripts/versao.mjs definir 0.2.0              → grava a versão (Cargo.toml, Cargo.lock,
//                                                         tauri.conf.json, package.json) e fecha a
//                                                         seção "Não publicado" do CHANGELOG
//   node scripts/versao.mjs notas 0.2.0                → texto do CHANGELOG daquela versão
// Opção: --raiz <pasta> (padrão: a raiz do repositório).
import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const iRaiz = args.indexOf("--raiz");
const raiz = iRaiz >= 0 ? path.resolve(args.splice(iRaiz, 2)[1]) : path.resolve(import.meta.dirname, "..");
const [cmd, ...resto] = args;

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;
const ARQ = {
  cargo: "Cargo.toml",
  lock: "Cargo.lock",
  tauri: "apps/desktop/src-tauri/tauri.conf.json",
  pacotes: ["package.json", "apps/desktop/package.json"],
  changelog: "CHANGELOG.md",
};

// Lê sempre em LF e grava de volta com a quebra de linha original do arquivo (no Windows o checkout
// pode trazer CRLF).
const crlf = new Set();
const ler = (p) => {
  const s = fs.readFileSync(path.join(raiz, p), "utf8");
  if (s.includes("\r\n")) crlf.add(p);
  return s.replace(/\r\n/g, "\n");
};
const gravar = (p, s) => fs.writeFileSync(path.join(raiz, p), crlf.has(p) ? s.replace(/\n/g, "\r\n") : s);
const falha = (m) => {
  console.error(`erro: ${m}`);
  process.exit(1);
};

function numero(v) {
  const n = String(v ?? "").replace(/^v/, "");
  if (!SEMVER.test(n)) falha(`versão "${v}" inválida: use X.Y.Z (ou vX.Y.Z)`);
  return n;
}

function atual() {
  const bloco = ler(ARQ.cargo).match(/\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/);
  if (!bloco) falha("versão não encontrada em [workspace.package] do Cargo.toml");
  return bloco[1];
}

function proxima(parte, base) {
  const [maj, min, pat] = numero(base).split(".").map(Number);
  if (parte === "major") return `${maj + 1}.0.0`;
  if (parte === "minor") return `${maj}.${min + 1}.0`;
  if (parte === "patch") return `${maj}.${min}.${pat + 1}`;
  return falha(`incremento "${parte}" inválido: use patch, minor ou major`);
}

// Pacotes do próprio workspace no Cargo.lock: os que não têm "source" (vêm de caminho local).
function blocosDoWorkspace(lock) {
  return lock.split(/(?=\[\[package\]\]\n)/).map((b) => ({ texto: b, nosso: b.startsWith("[[package]]") && !/\nsource = /.test(b) }));
}

function versoes() {
  const out = { [ARQ.cargo]: atual() };
  out[ARQ.tauri] = JSON.parse(ler(ARQ.tauri)).version;
  for (const p of ARQ.pacotes) out[p] = JSON.parse(ler(p)).version;
  const lock = blocosDoWorkspace(ler(ARQ.lock)).filter((b) => b.nosso);
  const nomes = new Set(lock.map((b) => b.texto.match(/\nversion = "([^"]+)"/)?.[1]));
  out[ARQ.lock] = nomes.size === 1 ? [...nomes][0] : [...nomes].join(" / ");
  return out;
}

function conferir(v) {
  const n = numero(v);
  const errados = Object.entries(versoes()).filter(([, x]) => x !== n);
  if (errados.length) falha(`versão diferente de ${n} em: ${errados.map(([a, x]) => `${a} (${x})`).join(", ")}`);
  console.log(`todos os arquivos estão em ${n}`);
}

function trocarVersaoJson(p, n) {
  const s = ler(p);
  const novo = s.replace(/("version"\s*:\s*")[^"]*(")/, `$1${n}$2`);
  if (novo === s && JSON.parse(s).version !== n) falha(`campo "version" não encontrado em ${p}`);
  gravar(p, novo);
}

function definir(v) {
  const n = numero(v);
  const cargo = ler(ARQ.cargo);
  gravar(ARQ.cargo, cargo.replace(/(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/, `$1${n}$2`));
  trocarVersaoJson(ARQ.tauri, n);
  for (const p of ARQ.pacotes) trocarVersaoJson(p, n);
  const lock = blocosDoWorkspace(ler(ARQ.lock))
    .map((b) => (b.nosso ? b.texto.replace(/\nversion = "[^"]+"/, `\nversion = "${n}"`) : b.texto))
    .join("");
  gravar(ARQ.lock, lock);
  fecharChangelog(n);
  conferir(n);
}

// "## Não publicado" vira "## X.Y.Z — data", e uma seção "Não publicado" vazia fica no topo.
function fecharChangelog(n) {
  if (!fs.existsSync(path.join(raiz, ARQ.changelog))) return;
  const s = ler(ARQ.changelog);
  if (new RegExp(`^## ${n.replace(/\./g, "\\.")}\\b`, "m").test(s)) return;
  const data = new Date().toISOString().slice(0, 10);
  const titulo = /^## Não publicado[ \t]*$/m;
  if (!titulo.test(s)) return;
  gravar(ARQ.changelog, s.replace(titulo, `## Não publicado\n\n## ${n} — ${data}`));
}

function notas(v) {
  const n = numero(v);
  const s = ler(ARQ.changelog).replace(/\r\n/g, "\n");
  const inicio = s.search(new RegExp(`^## ${n.replace(/\./g, "\\.")}\\b.*$`, "m"));
  if (inicio < 0) return console.log(`Versão ${n}. Veja o CHANGELOG.md.`);
  const corpo = s.slice(inicio).split("\n").slice(1);
  const fim = corpo.findIndex((l) => l.startsWith("## "));
  console.log((fim < 0 ? corpo : corpo.slice(0, fim)).join("\n").trim());
}

switch (cmd) {
  case "atual":
    console.log(atual());
    break;
  case "proxima":
    console.log(proxima(resto[0] ?? "patch", resto[1] ?? atual()));
    break;
  case "conferir":
    conferir(resto[0]);
    break;
  case "definir":
    definir(resto[0]);
    break;
  case "notas":
    notas(resto[0]);
    break;
  default:
    falha("use: atual | proxima <patch|minor|major> [base] | conferir <versão> | definir <versão> | notas <versão>");
}
