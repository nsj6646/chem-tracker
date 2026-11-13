export const SCALE_FACTOR = 10_000;
export const HUNDRED_PERCENT_SCALED = 1_000_000;

export type ParseDecimalResult =
  | { ok: true; scaled: number; canonical: string }
  | { ok: false; error: string };

/**
 * Losslessly parses a decimal string or number (up to 4 decimal places)
 * into a ×10,000 scaled integer and canonical normalized string.
 */
export function parseScaled4dp(input: string | number): ParseDecimalResult {
  const raw = String(input ?? "")
    .trim()
    .replaceAll(",", "");
  if (!raw) {
    return { ok: false, error: "숫자 값이 비어 있습니다." };
  }

  const match = /^([+-])?(?:(\d+)(?:\.(\d*))?|\.(\d+))$/.exec(raw);
  if (!match) {
    return { ok: false, error: `올바른 숫자 형식이 아닙니다: '${input}'` };
  }

  const sign = match[1] === "-" ? -1 : 1;
  const intDigits = (match[2] ?? "0").replace(/^0+(?=\d)/, "");
  const fracRaw = match[3] ?? match[4] ?? "";
  const fracTrimmed = fracRaw.replace(/0+$/, "");

  if (fracTrimmed.length > 4) {
    return {
      ok: false,
      error: `소수점 4자리 이하만 입력할 수 있습니다: '${input}'`,
    };
  }

  const fracPadded = (fracTrimmed + "0000").slice(0, 4);
  const scaledAbs = Number(intDigits) * SCALE_FACTOR + Number(fracPadded);
  if (!Number.isSafeInteger(scaledAbs)) {
    return { ok: false, error: "수량 범위가 허용치를 초과했습니다." };
  }

  const scaled = scaledAbs === 0 ? 0 : sign * scaledAbs;
  const canonicalBase =
    fracTrimmed.length > 0 ? `${intDigits}.${fracTrimmed}` : intDigits;
  const canonical =
    scaled < 0 ? `-${canonicalBase}` : canonicalBase;

  return { ok: true, scaled, canonical };
}

/**
 * Exact fixed-point calculation of substance weight (×10,000 scaled)
 * from product import weight (×10,000) and weight percentage (×10,000),
 * rounded MidpointAwayFromZero to 4 decimal places.
 */
export function computeSubstanceKgScaled(
  importKgScaled: number,
  pctScaled: number,
): number {
  const numerator = BigInt(importKgScaled) * BigInt(pctScaled);
  const divisor = BigInt(HUNDRED_PERCENT_SCALED);
  const absNum = numerator < 0n ? -numerator : numerator;
  const q = absNum / divisor;
  const rem = absNum % divisor;
  const rounded = rem * 2n >= divisor ? q + 1n : q;
  return Number(numerator < 0n ? -rounded : rounded);
}

export function formatPct(scaled4dp: number): string {
  return formatFixedScaled(scaled4dp, 4, 2, 4, false);
}

export function formatKg(scaled4dp: number): string {
  return formatFixedScaled(scaled4dp, 4, 2, 4, true);
}

export function formatTonFromKgScaled(kgScaled4dp: number): string {
  const scaledTon4 = roundHalfAway(BigInt(kgScaled4dp), 1000n);
  if (kgScaled4dp !== 0 && scaledTon4 === 0n) {
    const scaledTon5 = roundHalfAway(BigInt(kgScaled4dp), 100n);
    return formatBigIntScaled(scaledTon5, 5, 5, 5, true);
  }
  return formatBigIntScaled(scaledTon4, 4, 4, 4, true);
}

function roundHalfAway(value: bigint, divisor: bigint): bigint {
  const neg = value < 0n;
  const abs = neg ? -value : value;
  const q = abs / divisor;
  const rem = abs % divisor;
  const rounded = rem * 2n >= divisor ? q + 1n : q;
  return neg && rounded !== 0n ? -rounded : rounded;
}

function formatFixedScaled(
  scaled: number,
  scale: number,
  minDp: number,
  maxDp: number,
  groupThousands: boolean,
): string {
  return formatBigIntScaled(BigInt(scaled), scale, minDp, maxDp, groupThousands);
}

function formatBigIntScaled(
  scaled: bigint,
  scale: number,
  minDp: number,
  maxDp: number,
  groupThousands: boolean,
): string {
  const neg = scaled < 0n;
  const abs = neg ? -scaled : scaled;
  const factor = 10n ** BigInt(scale);
  const intPart = abs / factor;
  const fracVal = abs % factor;

  const intStr = groupThousands ? insertCommas(intPart.toString()) : intPart.toString();
  const fracFull = fracVal.toString().padStart(scale, "0").slice(0, maxDp);
  let fracTrimmed = fracFull.replace(/0+$/, "");
  while (fracTrimmed.length < minDp) {
    fracTrimmed += "0";
  }

  const sign = neg ? "-" : "";
  return fracTrimmed.length > 0 ? `${sign}${intStr}.${fracTrimmed}` : `${sign}${intStr}`;
}

function insertCommas(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}
