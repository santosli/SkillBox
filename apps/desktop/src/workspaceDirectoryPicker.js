export const workspaceDirectoryPickerOptions = Object.freeze({
  directory: true,
  multiple: false,
  recursive: false,
  canCreateDirectories: false,
  title: 'Choose project or skills folder'
});

export async function chooseWorkspaceDirectory(openDialog) {
  const selected = await openDialog(workspaceDirectoryPickerOptions);
  if (selected === null) return null;
  if (typeof selected !== 'string' || !selected.trim()) {
    throw new Error('The directory picker returned an invalid path.');
  }
  return selected;
}
