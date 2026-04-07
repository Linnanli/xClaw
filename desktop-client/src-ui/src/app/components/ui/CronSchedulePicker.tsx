/**
 * CronSchedulePicker — 频率选择器
 *
 * 将"每天/每周/每月 + 时间"的用户选择转换为标准 5 字段 cron 表达式。
 * 纯受控组件，不持有内部状态，所有变更通过 onChange 回调通知父组件。
 */

import CronTime from 'cron-time-generator';
import cronstrue from 'cronstrue/i18n';
import { cn } from './utils';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './select';

// ─── 类型定义 ────────────────────────────────────────────────────────

export type FrequencyType = 'daily' | 'weekly' | 'monthly';

export interface CronSchedule {
  frequency: FrequencyType;
  hour: number;
  minute: number;
  /** 周几（0=周日, 1=周一 … 6=周六），仅 weekly 有效 */
  weekday: number;
  /** 几号（1-28），仅 monthly 有效 */
  monthDay: number;
}

export interface CronSchedulePickerProps {
  value: CronSchedule;
  onChange: (schedule: CronSchedule) => void;
  className?: string;
}

// ─── 常量 ────────────────────────────────────────────────────────────

const FREQUENCY_OPTIONS: { value: FrequencyType; label: string }[] = [
  { value: 'daily', label: '每天' },
  { value: 'weekly', label: '每周' },
  { value: 'monthly', label: '每月' },
];

const WEEKDAY_OPTIONS = [
  { value: 1, label: '周一' },
  { value: 2, label: '周二' },
  { value: 3, label: '周三' },
  { value: 4, label: '周四' },
  { value: 5, label: '周五' },
  { value: 6, label: '周六' },
  { value: 0, label: '周日' },
];

const HOURS = Array.from({ length: 24 }, (_, i) => i);
const MINUTES = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];
const MONTH_DAYS = Array.from({ length: 28 }, (_, i) => i + 1);

// ─── 转换函数 ────────────────────────────────────────────────────────

/** 将 CronSchedule 转换为标准 5 字段 cron 表达式 */
export function toCronExpression(schedule: CronSchedule): string {
  const { frequency, hour, minute, weekday, monthDay } = schedule;
  switch (frequency) {
    case 'daily':
      return CronTime.everyDayAt(hour, minute);
    case 'weekly':
      return CronTime.onSpecificDaysAt([weekday], hour, minute);
    case 'monthly':
      return `${minute} ${hour} ${monthDay} * *`;
  }
}

/** 将 cron 表达式转换为人类可读描述 */
export function cronToDescription(expression: string): string {
  try {
    return cronstrue.toString(expression, { locale: 'zh_CN' });
  } catch {
    return expression;
  }
}

/** 默认的 CronSchedule 初始值 */
export const DEFAULT_CRON_SCHEDULE: CronSchedule = {
  frequency: 'daily',
  hour: 9,
  minute: 0,
  weekday: 1,
  monthDay: 1,
};

// ─── 主组件 ──────────────────────────────────────────────────────────

export function CronSchedulePicker({ value, onChange, className }: CronSchedulePickerProps) {
  const update = (patch: Partial<CronSchedule>) => onChange({ ...value, ...patch });

  const hourOptions = HOURS.map((h) => ({
    value: String(h),
    label: `${h.toString().padStart(2, '0')} 时`,
  }));
  const minuteOptions = MINUTES.map((m) => ({
    value: String(m),
    label: `${m.toString().padStart(2, '0')} 分`,
  }));
  const monthDayOptions = MONTH_DAYS.map((d) => ({
    value: String(d),
    label: `${d} 号`,
  }));
  const weekdayOptions = WEEKDAY_OPTIONS.map((w) => ({
    value: String(w.value),
    label: w.label,
  }));

  return (
    <div className={cn('space-y-2.5', className)}>
      {/* 频率选择 */}
      <div className="flex gap-1.5">
        {FREQUENCY_OPTIONS.map(({ value: freq, label }) => (
          <button
            key={freq}
            type="button"
            onClick={() => update({ frequency: freq })}
            className={cn(
              'flex-1 rounded-lg border px-3 py-1.5 text-sm font-medium transition-colors',
              value.frequency === freq
                ? 'border-primary bg-primary/10 text-primary'
                : 'border-border bg-secondary text-muted-foreground hover:bg-accent',
            )}
          >
            {label}
          </button>
        ))}
      </div>

      {/* 时间选择行 */}
      <div className="flex flex-wrap items-center gap-2">
        {value.frequency === 'weekly' && (
          <Select
            value={String(value.weekday)}
            onValueChange={(v) => update({ weekday: Number(v) })}
          >
            <SelectTrigger size="sm" className="w-auto min-w-[80px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {weekdayOptions.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        {value.frequency === 'monthly' && (
          <Select
            value={String(value.monthDay)}
            onValueChange={(v) => update({ monthDay: Number(v) })}
          >
            <SelectTrigger size="sm" className="w-auto min-w-[72px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {monthDayOptions.map((o) => (
                <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        <Select
          value={String(value.hour)}
          onValueChange={(v) => update({ hour: Number(v) })}
        >
          <SelectTrigger size="sm" className="w-auto min-w-[72px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {hourOptions.map((o) => (
              <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={String(value.minute)}
          onValueChange={(v) => update({ minute: Number(v) })}
        >
          <SelectTrigger size="sm" className="w-auto min-w-[72px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {minuteOptions.map((o) => (
              <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* 预览 */}
      <p className="text-[11px] text-muted-foreground">
        {cronToDescription(toCronExpression(value))}
      </p>
    </div>
  );
}
