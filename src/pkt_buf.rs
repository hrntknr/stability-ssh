use std::collections::VecDeque;

pub struct DataBuf {
    pub buf: VecDeque<Vec<u8>>,
}

pub fn to_pkt(id: u32, data: Vec<u8>) -> Vec<u8> {
    let mut buf = vec![];
    buf.extend(id.to_be_bytes().iter());
    buf.extend((data.len() as u16).to_be_bytes().iter());
    buf.extend(data);
    buf
}

impl DataBuf {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::new(),
        }
    }

    pub fn push(&mut self, pkt: Vec<u8>) {
        self.buf.push_back(pkt);
    }
}

impl Iterator for DataBuf {
    type Item = (u32, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut id: Option<u32> = None;
        let mut len: Option<u16> = None;
        let mut buf = vec![];
        loop {
            match self.buf.pop_front() {
                Some(d) => buf.extend(d),
                None => break,
            }

            if buf.len() < 6 {
                continue;
            }
            if id.is_none() {
                id = Some(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]));
            }
            if len.is_none() {
                len = Some(u16::from_be_bytes([buf[4], buf[5]]));
            }
            if buf.len() >= len.unwrap() as usize + 6 {
                buf = buf.split_off(6);
                let d = buf.split_off(len.unwrap() as usize);
                if d.len() > 0 {
                    self.buf.push_front(d);
                }
                return Some((id.unwrap(), buf));
            }
        }

        if buf.len() > 0 {
            self.buf.push_front(buf);
        }
        None
    }
}

pub struct AckBuf {
    pub buf: VecDeque<Vec<u8>>,
}

pub fn to_ack_pkt(id: u32) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

impl AckBuf {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::new(),
        }
    }

    pub fn push(&mut self, pkt: Vec<u8>) {
        self.buf.push_back(pkt);
    }
}

impl Iterator for AckBuf {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![];
        loop {
            match self.buf.pop_front() {
                Some(d) => buf.extend(d),
                None => break,
            }

            if buf.len() < 4 {
                continue;
            }
            let id = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            return Some(id);
        }
        None
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_buf() {
        let mut buf = super::DataBuf::new();

        buf.push(vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x01]);
        assert_eq!(buf.next(), None);
        buf.push(vec![0x00]);
        assert_eq!(buf.next(), Some((1, vec![0x00])));
        assert_eq!(buf.next(), None);
        assert_eq!(buf.buf.len(), 0);

        buf.push(vec![0x00, 0x00, 0x00, 0x02, 0x00, 0x00]);
        assert_eq!(buf.next(), Some((2, vec![])));
        assert_eq!(buf.next(), None);
        assert_eq!(buf.buf.len(), 0);

        buf.push(vec![0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x00]);
        assert_eq!(buf.next(), Some((3, vec![0x00])));
        assert_eq!(buf.next(), None);
        assert_eq!(buf.buf.len(), 1);
    }
}
