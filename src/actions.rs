//! Application actions and their key bindings.

use gpui_kit::actions;

actions!(
    android_files,
    [
        OpenSettings,
        Refresh,
        NavigateUp,
        ToggleHidden,
        ToggleHostPane,
        DeleteSelected,
        CopySelected,
        CutSelected,
        Paste,
        RenameSelected,
        NewFolder,
        NewFile,
        ShowProperties,
        FocusSearch,
        GoBack,
        GoForward
    ]
);
