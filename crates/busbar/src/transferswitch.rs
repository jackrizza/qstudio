#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferSwitchPosition {
    A,
    B,
}

#[derive(Debug, Clone)]
pub struct TransferSwitch<T> {
    a: T,
    b: T,
    ptr: TransferSwitchPosition,
}

impl<T> TransferSwitch<T> {
    pub fn new(a: T, b: T) -> Self {
        TransferSwitch {
            a,
            b,
            ptr: TransferSwitchPosition::A,
        }
    }

    pub fn switch(&mut self) {
        self.ptr = match self.ptr {
            TransferSwitchPosition::A => TransferSwitchPosition::B,
            TransferSwitchPosition::B => TransferSwitchPosition::A,
        };
    }
    pub fn read(&self) -> &T {
        match self.ptr {
            TransferSwitchPosition::A => &self.a,
            TransferSwitchPosition::B => &self.b,
        }
    }

    pub fn write(&mut self, value: T) {
        match self.ptr {
            TransferSwitchPosition::A => self.a = value,
            TransferSwitchPosition::B => self.b = value,
        }
    }
}
