-- ═══════════════════════════════════════════════════════════
-- 椰果工坊 · Buff 集示例导出（用于演示「SQL 导入 Buff 集」）
-- 由 /api/buff-sets/export 导出的格式；可用 `lite sql --path` 或 TUI 的
-- 「SQL 导入 Buff 集」菜单直接导入。
-- ═══════════════════════════════════════════════════════════

BEGIN;
INSERT INTO public.buff_sets (entity_type, entity_name, buff_name, scope, exclusive, condition, buff_set) VALUES
('character','今汐','渊鳞','self',false, NULL, '[{"zoneId":"bonusDmg","value":10.0}]'::jsonb),
('character','维里奈','共济','team',false, NULL, '[{"zoneId":"atkPct","value":12.0},{"zoneId":"recharge","value":10.0}]'::jsonb),
('character','卡卡罗','深痕','self',false, '{"chain":3,"elements":["导电"]}'::jsonb, '[{"zoneId":"critDmg","value":12.0}]'::jsonb),
('weapon','千古洑流','剑心','self',false, NULL, '[{"zoneId":"atkPct","value":12.0},{"zoneId":"bonusDmg","value":12.0}]'::jsonb),
('5set','轻云出月','五件套','team',false, NULL, '[{"zoneId":"recharge","value":10.0}]'::jsonb),
('echo','无常凶鹭','主位','self',false, NULL, '[{"zoneId":"bonusDmg","value":12.0}]'::jsonb);
COMMIT;
