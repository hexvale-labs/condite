export type ExpiryChoice = {
  mode: "default" | "never" | "date" | "interval" | "upload";
  date: string;
  days: string;
  subtract: boolean;
};
export const defaultExpiry = (): ExpiryChoice => ({
  mode: "default",
  date: "",
  days: "14",
  subtract: false,
});

/** datetime-local is deliberately interpreted as UTC, matching the field label. */
export function resolveExpiry(
  choice: ExpiryChoice,
  base?: string | null,
  now = Date.now(),
): string | null | undefined {
  if (choice.mode === "default" || choice.mode === "upload") return undefined;
  if (choice.mode === "never") return null;
  let value: number;
  if (choice.mode === "date") {
    if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d$/.test(choice.date))
      throw new Error("Invalid expiry");
    value = Date.parse(`${choice.date}:00Z`);
    if (
      !Number.isFinite(value) ||
      new Date(value).toISOString().slice(0, 16) !== choice.date
    )
      throw new Error("Invalid expiry");
  } else {
    const days = Number(choice.days);
    if (!Number.isInteger(days) || days <= 0 || days > 36500)
      throw new Error("Invalid interval");
    value =
      (base ? Date.parse(base) : now) +
      days * 86400000 * (choice.subtract ? -1 : 1);
  }
  if (!Number.isFinite(value) || value > 8640000000000000 || value < 0)
    throw new Error("Invalid expiry");
  return new Date(value).toISOString();
}
