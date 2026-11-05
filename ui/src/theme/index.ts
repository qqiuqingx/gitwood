export type ThemeChoice = 'system' | 'light' | 'dark' | 'walnut' | `user:${string}`;
export interface ThemeError { code: string; path: string; message: string }
export interface Theme { name: string; dark: boolean; variables: Record<string, string>; errors: ThemeError[] }
export interface ContrastIssue { foreground: string; background: string; ratio: number; minimum: number }
export interface ThemeDescriptor { id: Exclude<ThemeChoice, 'system'>; name: string; dark: boolean; builtIn: boolean; contrastIssues: ContrastIssue[] }
export interface ThemeFileError { fileName: string; errors: ThemeError[] }
export interface ThemeCatalog { directory: string; themes: ThemeDescriptor[]; errors: ThemeFileError[] }
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
