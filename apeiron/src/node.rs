use smallvec::SmallVec;

/// Node address in the arena. u32::MAX is the NONE sentinel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ptr(pub u32);

impl Ptr {
    pub const NONE: Ptr = Ptr(u32::MAX);

    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A single port on a node: where does this wire go?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Port {
    pub target: Ptr,
    pub slot: u8,
}

impl Port {
    pub fn new(target: Ptr, slot: u8) -> Self {
        Port { target, slot }
    }

    pub fn disconnected() -> Self {
        Port {
            target: Ptr::NONE,
            slot: 0,
        }
    }

    pub fn is_connected(&self) -> bool {
        !self.target.is_none()
    }
}

/// The opcode of a node — what kind of interaction net agent it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpCode {
    /// Lambda: 0=Principal, 1=Var, 2=Body
    Lam,
    /// Application: 0=Principal, 1=Arg, 2=Result
    App,
    /// Eraser: 0=Principal (only port). Absorbs anything.
    Erase,
    /// Duplicator: 0=Principal, 1=CopyA, 2=CopyB
    Dup { label: u32 },
    /// Contextual barrier: 0=Principal, 1=Inner
    Barrier { scope: u32 },
    /// Unification variable / meta-variable: 0=Principal. Suspends evaluation.
    Future,
    /// Named constant/symbol: 0=Principal, then N aux ports for arguments
    Sym { name: String, arity: u8 },
}

impl OpCode {
    /// How many ports does this opcode need?
    pub fn port_count(&self) -> usize {
        match self {
            OpCode::Lam => 3,        // principal, var, body
            OpCode::App => 3,        // principal, arg, result
            OpCode::Erase => 1,      // principal only
            OpCode::Dup { .. } => 3, // principal, copyA, copyB
            OpCode::Barrier { .. } => 2, // principal, inner
            OpCode::Future => 1,         // principal only
            OpCode::Sym { arity, .. } => 1 + *arity as usize, // principal + args
        }
    }
}

/// A node in the interaction net.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: Ptr,
    pub kind: OpCode,
    /// Port 0 is always the principal port. Ports 1..n are auxiliary.
    pub ports: SmallVec<[Port; 4]>,
}

impl Node {
    pub fn new(id: Ptr, kind: OpCode) -> Self {
        let num_ports = kind.port_count();
        let mut ports = SmallVec::new();
        for _ in 0..num_ports {
            ports.push(Port::disconnected());
        }
        Node {
            id,
            kind,
            ports,
        }
    }
}
