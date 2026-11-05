import { effectiveTheme, type ThemeChoice, type ThemeDescriptor, type ThemeFileError, type ThemeError } from './index';

export interface ThemeErrorRow { fileName: string; error: ThemeError }

/** UI actions use the same callbacks as the dialog for preview, cancel, and save. */
export class ThemeSettingsSession {
  selected: ThemeChoice;
  changed = false;

  constructor(
    readonly original: ThemeChoice,
    private readonly preview: (choice: ThemeChoice) => Promise<void>,
    private readonly save: (choice: ThemeChoice) => Promise<void>,
  ) {
    this.selected = original;
  }

  async choose(choice: ThemeChoice): Promise<void> {
    this.selected = choice;
    this.changed = true;
    await this.preview(choice);
  }

  async cancel(): Promise<void> {
    await this.preview(this.original);
  }

  async confirm(): Promise<boolean> {
    if (!this.changed) return false;
    await this.save(this.selected);
    return true;
  }
}

export function themeAvailable(choice: ThemeChoice, themes: ThemeDescriptor[]): boolean {
  return choice === 'system' || themes.some(theme => theme.id === choice);
}

/** Keep the saved ID separate from the radio choice while that theme is unavailable. */
export function displayedChoice(choice: ThemeChoice, themes: ThemeDescriptor[]): ThemeChoice {
  return themeAvailable(choice, themes) ? choice : 'system';
}

export function errorRows(errors: ThemeFileError[]): ThemeErrorRow[] {
  return errors.flatMap(({ fileName, errors: fileErrors }) => fileErrors.map(error => ({ fileName, error })));
}

export function contrastWarnings(
  choice: ThemeChoice,
  themes: ThemeDescriptor[],
  systemDark: boolean,
): ThemeDescriptor['contrastIssues'] {
  const id = effectiveTheme(choice, systemDark);
  return themes.find(theme => theme.id === id)?.contrastIssues ?? [];
}
