export type ThemeChoice = 'system' | 'light' | 'dark' | 'walnut';
export interface ThemeError { code: string; path: string; message: string }
export interface Theme { name: string; dark: boolean; variables: Record<string, string>; errors: ThemeError[] }
let appliedVariables: string[] = [];

/** Apply the validated map through CSSOM without replacing any UI or repository state. */
export function applyTheme(theme: Theme): void {
  const style = document.documentElement.style;
  for (const key of appliedVariables) {
    if (!(key in theme.variables)) style.removeProperty(key);
  }
  for (const [key, value] of Object.entries(theme.variables)) style.setProperty(key, value);
  style.setProperty('color-scheme', theme.dark ? 'dark' : 'light');
  appliedVariables = Object.keys(theme.variables);
}
export function effectiveTheme(choice: ThemeChoice, dark: boolean): Exclude<ThemeChoice, 'system'> {
  return choice === 'system' ? dark ? 'dark' : 'light' : choice;
}
