use anyhow::Result;

pub struct Queue {
    q: std::collections::VecDeque<Vec<u8>>,
    head: u32,
    max: u32,
}

impl Queue {
    pub fn new(max: u32) -> Self {
        Self {
            q: std::collections::VecDeque::new(),
            head: 0,
            max,
        }
    }

    pub fn check(&mut self, idx: u32) -> Result<()> {
        let cnt = (self.max + idx - self.head) % self.max + 1;
        if self.len() < cnt {
            return Err(anyhow::anyhow!("invalid idx"));
        }
        for _ in 0..cnt {
            self.q.pop_front();
        }
        self.head = (self.head + cnt) % self.max;

        Ok(())
    }

    pub fn push(&mut self, buf: Vec<u8>) -> Result<u32> {
        if self.len() == self.max {
            return Err(anyhow::anyhow!("full"));
        }
        self.q.push_back(buf);

        let idx = (self.head + self.len() - 1) % self.max;
        Ok(idx)
    }

    pub fn len(&self) -> u32 {
        self.q.len() as u32
    }
    pub fn head(&self) -> u32 {
        self.head
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_queue() {
        let mut q = super::Queue::new(4);
        assert!(matches!(q.push(vec![1]), Ok(0)));
        assert_eq!(q.len(), 1);
        assert_eq!(q.head(), 0);
        assert!(matches!(q.check(0), Ok(())));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.check(1), Err(_)));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.push(vec![2]), Ok(1)));
        assert!(matches!(q.push(vec![3]), Ok(2)));
        assert!(matches!(q.push(vec![4]), Ok(3)));
        assert!(matches!(q.push(vec![5]), Ok(0)));
        assert_eq!(q.len(), 4);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.push(vec![6]), Err(_)));
        assert_eq!(q.len(), 4);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.check(0), Ok(())));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 1);
        assert!(matches!(q.check(1), Err(_)));
        assert_eq!(q.len(), 0);
        assert_eq!(q.head(), 1);
    }
}
