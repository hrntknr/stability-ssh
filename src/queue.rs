use anyhow::Result;

pub struct Queue {
    q: std::collections::VecDeque<Vec<u8>>,
    head: u32,
    max: u32,
}

impl Queue {
    pub fn new(bit: u8) -> Self {
        if bit > 32 {
            panic!("bit too large");
        }
        Self {
            q: std::collections::VecDeque::new(),
            head: 1,
            max: 2u32.wrapping_pow(bit as u32).wrapping_sub(1),
        }
    }
    pub fn add(&self, a: u32, b: u32) -> u32 {
        a.wrapping_add(b) & self.max
    }
    pub fn sub(&self, a: u32, b: u32) -> u32 {
        a.wrapping_sub(b) & self.max
    }
    pub fn len(&self) -> u32 {
        self.q.len() as u32
    }
    pub fn head(&self) -> u32 {
        self.head
    }

    pub fn vidx(&self, idx: u32) -> u32 {
        self.add(idx, self.head)
    }
    pub fn idx(&self, vidx: u32) -> u32 {
        self.sub(vidx, self.head)
    }

    pub fn push(&mut self, buf: Vec<u8>) -> Result<u32> {
        if self.len() > self.max {
            return Err(anyhow::anyhow!("full"));
        }
        self.q.push_back(buf);
        Ok(self.vidx(self.len() - 1))
    }

    pub fn check(&mut self, vidx: u32) -> Result<()> {
        let idx = self.idx(vidx);
        if self.len() <= idx {
            return Err(anyhow::anyhow!("invalid idx"));
        }
        for _ in 0..=idx {
            self.q.pop_front();
        }
        self.head = self.add(vidx, 1);

        Ok(())
    }

    pub fn list(&self, vidx: u32) -> Result<Vec<(u32, Vec<u8>)>> {
        let idx = self.add(self.idx(vidx), 1);
        if self.len() < idx {
            return Err(anyhow::anyhow!("invalid idx"));
        }
        let mut ret = Vec::new();
        for i in idx..self.len() {
            let vidx = self.add(self.head, i);
            let buf = self.q.get(i as usize).unwrap().clone();
            ret.push((vidx, buf));
        }
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_add_sub() {
        let q = super::Queue::new(2);
        assert_eq!(q.add(0, 1), 1);
        assert_eq!(q.add(1, 1), 2);
        assert_eq!(q.add(1, 2), 3);
        assert_eq!(q.add(1, 3), 0);
        assert_eq!(q.sub(0, 1), 3);
        let q2 = super::Queue::new(32);
        assert_eq!(q2.add(u32::MAX, 1), 0);
        assert_eq!(q2.add(u32::MAX, u32::MAX), u32::MAX - 1);
    }
    #[test]
    fn test_queue() {
        let mut q = super::Queue::new(2);
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 1);
        assert_eq!(q.list(0).unwrap().len(), 0);
        assert!(matches!(q.list(1), Err(_)));
        assert!(matches!(q.list(2), Err(_)));
        assert!(matches!(q.list(3), Err(_)));
        assert!(matches!(q.push(vec![1]), Ok(1)));
        assert_eq!(q.len(), 1);
        assert_eq!(q.head(), 1);
        assert_eq!(q.list(0).unwrap().len(), 1);
        assert_eq!(q.list(0).unwrap()[0].0, 1);
        assert_eq!(q.list(1).unwrap().len(), 0);
        assert!(matches!(q.list(2), Err(_)));
        assert!(matches!(q.list(3), Err(_)));
        assert!(matches!(q.check(1), Ok(())));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 2);
        assert!(matches!(q.list(0), Err(_)));
        assert_eq!(q.list(1).unwrap().len(), 0);
        assert!(matches!(q.list(2), Err(_)));
        assert!(matches!(q.list(3), Err(_)));
        assert!(matches!(q.check(2), Err(_)));
        assert!(matches!(q.push(vec![2]), Ok(2)));
        assert!(matches!(q.push(vec![3]), Ok(3)));
        assert!(matches!(q.push(vec![4]), Ok(0)));
        assert!(matches!(q.push(vec![5]), Ok(1)));
        assert_eq!(q.len(), 4);
        assert_eq!(q.head(), 2);
        assert_eq!(q.list(1).unwrap().len(), 4);
        assert_eq!(q.list(1).unwrap()[0].0, 2);
        assert_eq!(q.list(2).unwrap().len(), 3);
        assert_eq!(q.list(2).unwrap()[0].0, 3);
        assert_eq!(q.list(3).unwrap().len(), 2);
        assert_eq!(q.list(3).unwrap()[0].0, 0);
        assert_eq!(q.list(0).unwrap().len(), 1);
        assert_eq!(q.list(0).unwrap()[0].0, 1);
        assert!(matches!(q.push(vec![6]), Err(_)));
        assert_eq!(q.len(), 4);
        assert_eq!(q.head(), 2);
        assert!(matches!(q.check(1), Ok(())));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 2);
        assert!(matches!(q.list(0), Err(_)));
        assert_eq!(q.list(1).unwrap().len(), 0);
        assert!(matches!(q.list(2), Err(_)));
        assert!(matches!(q.list(3), Err(_)));
        assert!(matches!(q.check(2), Err(_)));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 2);
    }

    #[test]
    fn test_overflow() {
        let mut q = super::Queue::new(32);
        assert!(matches!(q.push(vec![1]), Ok(1)));
        assert_eq!(q.len(), 1);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.check(1), Ok(())));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 2);
        assert!(matches!(q.check(2), Err(_)));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 2);
        assert!(matches!(q.push(vec![2]), Ok(2)));
    }
}
