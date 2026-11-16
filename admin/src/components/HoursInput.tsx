import { InputNumber, Space } from 'antd';

export function HoursInput({
  value,
  onChange,
}: {
  value: number | null;
  onChange: (v: number) => void;
}) {
  const h = Math.trunc((value ?? 0) / 60);
  const m = (value ?? 0) % 60;
  return (
    <Space>
      <InputNumber min={0} value={h} onChange={(v) => onChange((v ?? 0) * 60 + m)} addonAfter="ч" />
      <InputNumber
        min={0}
        max={59}
        value={m}
        onChange={(v) => onChange(h * 60 + (v ?? 0))}
        addonAfter="м"
      />
    </Space>
  );
}
