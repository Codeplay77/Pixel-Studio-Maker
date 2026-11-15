// Escolha de um modelo do RPG Maker no diálogo "Novo projeto". Criar o projeto e aplicar o modelo
// são dois comandos do runtime (project_create + rpgmaker_template), os mesmos que o MCP usa.
import { BLANK_TEMPLATE, choiceSize, RPG_ENGINES, RPG_KINDS, type RpgKind, type RpgTemplateChoice } from "./templates";

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-muted">
      <span>{label}</span>
      {children}
    </label>
  );
}

export { BLANK_TEMPLATE, choiceSize };
export type { RpgTemplateChoice };

export function RpgMakerTemplateFields({ value, onChange }: { value: RpgTemplateChoice; onChange: (v: RpgTemplateChoice) => void }) {
  const set = (p: Partial<RpgTemplateChoice>) => {
    const next = { ...value, ...p };
    // O VX Ace não tem battler lateral.
    if (next.kind === "sv_battler" && next.engine === "vxace") next.engine = "mz";
    onChange(next);
  };
  const size = choiceSize(value);
  return (
    <>
      <Row label="Modelo">
        <select value={value.kind} onChange={(e) => set({ kind: e.target.value as RpgKind | "" })} className="w-56">
          <option value="">Em branco</option>
          <optgroup label="RPG Maker">
            {RPG_KINDS.map((k) => (
              <option key={k.id} value={k.id}>
                {k.label}
              </option>
            ))}
          </optgroup>
        </select>
      </Row>
      {value.kind && (
        <>
          <Row label="Motor">
            <select value={value.engine} onChange={(e) => set({ engine: e.target.value as RpgTemplateChoice["engine"] })} className="w-56">
              {RPG_ENGINES.map((en) => (
                <option key={en.id} value={en.id} disabled={value.kind === "sv_battler" && en.id === "vxace"}>
                  {en.label} ({en.id === "vxace" ? "tiles de 32 px" : "tiles de 48 px"})
                </option>
              ))}
            </select>
          </Row>
          {value.kind === "character" && (
            <Row label="Um personagem só (arquivo com $)">
              <input type="checkbox" checked={value.single} onChange={(e) => set({ single: e.target.checked })} />
            </Row>
          )}
          {value.kind === "window" && (
            <Row label="Pintar as 32 cores do texto">
              <input type="checkbox" checked={value.textColors} onChange={(e) => set({ textColors: e.target.checked })} />
            </Row>
          )}
          <p className="mb-2 text-[12px] leading-5 text-faint">
            {size
              ? `Tamanho fixo de ${size[0]}×${size[1]} px. Cada bloco vira uma região nomeada (veja com o botão de regiões no canvas).`
              : "Este motor não tem esse arquivo."}
          </p>
        </>
      )}
    </>
  );
}

type Run = (name: string, args?: Record<string, unknown>) => Promise<any>;

/** Cria o projeto no tamanho do modelo e aplica o modelo ao primeiro sprite. */
export async function createRpgMakerProject(run: Run, name: string, c: RpgTemplateChoice): Promise<void> {
  const size = choiceSize(c);
  if (!c.kind || !size) return;
  const created = await run("project_create", { name, width: size[0], height: size[1] });
  await run("rpgmaker_template", {
    kind: c.kind,
    engine: c.engine,
    sprite: created?.data?.sprite?.id,
    single: c.kind === "character" ? c.single : undefined,
    fill_text_colors: c.kind === "window" ? c.textColors : undefined,
  });
}
