// Monta a pasta godot/: um projeto Godot 4.3+ pronto para abrir, com uma cena Parallax2D reaproveitável
// (parallax.tscn) e uma cena de demonstração com câmera andando sozinha (demo_parallax.tscn).
// Os PNGs das camadas são exportados pelo MCP (gerar.mjs) em godot/camadas/.
// Uso (na pasta examples/): node demo-h-parallax/godot.mjs
import fs from "node:fs";
import path from "node:path";
import { CAMADAS } from "./gerar.mjs";

const dir = path.join(import.meta.dirname, "godot");
const W = 320;
const H = 180;
const nome = (s) => s.split("_").map((p) => p[0].toUpperCase() + p.slice(1)).join("");
const num = (v) => (Number.isInteger(v) ? String(v) : String(Number(v.toFixed(3))));

const ext = CAMADAS.map(([camada], i) => `[ext_resource type="Texture2D" path="res://camadas/${String(i + 1).padStart(2, "0")}_${camada}.png" id="${i + 1}_${camada}"]`);
const nos = CAMADAS.flatMap(([camada, vel], i) => [
  "",
  `[node name="${nome(camada)}" type="Parallax2D" parent="."]`,
  `scroll_scale = Vector2(${num(vel)}, ${num(vel)})`,
  `repeat_size = Vector2(${W}, 0)`,
  "repeat_times = 3",
  "",
  `[node name="Sprite2D" type="Sprite2D" parent="${nome(camada)}"]`,
  `texture = ExtResource("${i + 1}_${camada}")`,
  "centered = false",
]);

const parallax = [
  `[gd_scene load_steps=${CAMADAS.length + 1} format=3]`,
  "",
  ...ext,
  "",
  '[node name="CenarioParallax" type="Node2D"]',
  "texture_filter = 1",
  ...nos,
  "",
].join("\n");

const demo = `[gd_scene load_steps=3 format=3]

[ext_resource type="PackedScene" path="res://parallax.tscn" id="1_cenario"]
[ext_resource type="Script" path="res://camera_demo.gd" id="2_camera"]

[node name="Demo" type="Node2D"]

[node name="CenarioParallax" parent="." instance=ExtResource("1_cenario")]

[node name="Camera2D" type="Camera2D" parent="."]
anchor_mode = 0
script = ExtResource("2_camera")
`;

const camera = `extends Camera2D
## Câmera de demonstração: anda sozinha para a direita para mostrar o parallax.
## As setas esquerda/direita mudam a direção e aceleram.

@export var velocidade := 30.0


func _process(delta: float) -> void:
\tvar direcao := Input.get_axis("ui_left", "ui_right")
\tif direcao == 0.0:
\t\tposition.x += velocidade * delta
\telse:
\t\tposition.x += direcao * velocidade * 4.0 * delta
`;

const projeto = `; Projeto mínimo para ver o cenário em parallax (gerado pelo Pixel Studio Maker).
config_version=5

[application]

config/name="Cenário parallax"
run/main_scene="res://demo_parallax.tscn"
config/features=PackedStringArray("4.3")

[display]

window/size/viewport_width=${W}
window/size/viewport_height=${H}
window/size/window_width_override=${W * 4}
window/size/window_height_override=${H * 4}
window/stretch/mode="viewport"

[rendering]

textures/canvas_textures/default_texture_filter=0
2d/snap/snap_2d_transforms_to_pixel=true
2d/snap/snap_2d_vertices_to_pixel=true
environment/defaults/default_clear_color=Color(0.105882, 0.105882, 0.227451, 1)
`;

fs.writeFileSync(path.join(dir, "parallax.tscn"), parallax);
fs.writeFileSync(path.join(dir, "demo_parallax.tscn"), demo);
fs.writeFileSync(path.join(dir, "camera_demo.gd"), camera);
fs.writeFileSync(path.join(dir, "project.godot"), projeto);
console.log(`godot/: ${CAMADAS.length} camadas (${CAMADAS.map(([c, v]) => `${c} ${v}`).join(", ")})`);
