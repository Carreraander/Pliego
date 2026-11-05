//! Tipos de los campos de formulario (AcroForm); el campo en sí es [`crate::PdfWidget`], en `native`.

use std::ops::{BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetType {
    Unknown,
    Button,
    Checkbox,
    Combobox,
    Listbox,
    RadioButton,
    Signature,
    Text,
}

/// Indicadores `/Ff` del campo (combinables con `|`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldFlags(pub i32);

impl FieldFlags {
    pub const READ_ONLY: FieldFlags = FieldFlags(1 << 0);
    pub const REQUIRED: FieldFlags = FieldFlags(1 << 1);
    pub const NO_EXPORT: FieldFlags = FieldFlags(1 << 2);
    pub const MULTILINE: FieldFlags = FieldFlags(1 << 12);
    pub const PASSWORD: FieldFlags = FieldFlags(1 << 13);
    pub const NO_TOGGLE_TO_OFF: FieldFlags = FieldFlags(1 << 14);
    pub const RADIO: FieldFlags = FieldFlags(1 << 15);
    pub const PUSHBUTTON: FieldFlags = FieldFlags(1 << 16);
    pub const COMBO: FieldFlags = FieldFlags(1 << 17);
    pub const EDIT: FieldFlags = FieldFlags(1 << 18);
    pub const SORT: FieldFlags = FieldFlags(1 << 19);
    pub const FILE_SELECT: FieldFlags = FieldFlags(1 << 20);
    pub const MULTI_SELECT: FieldFlags = FieldFlags(1 << 21);
    pub const DO_NOT_SPELL_CHECK: FieldFlags = FieldFlags(1 << 22);
    pub const DO_NOT_SCROLL: FieldFlags = FieldFlags(1 << 23);
    pub const COMB: FieldFlags = FieldFlags(1 << 24);
    pub const RADIOS_IN_UNISON: FieldFlags = FieldFlags(1 << 25);
    pub const COMMIT_ON_SEL_CHANGE: FieldFlags = FieldFlags(1 << 26);
    pub const RICH_TEXT: FieldFlags = FieldFlags(1 << 25);

    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn bits(self) -> i32 {
        self.0
    }
    pub const fn contains(self, o: FieldFlags) -> bool {
        self.0 & o.0 == o.0
    }
}

impl BitOr for FieldFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for FieldFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
