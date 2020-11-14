use std::fmt::Debug;

#[derive(Debug)]
pub struct FixedRingBuffer<T> {
    buf: Vec<T>,
    cap: usize,
    head: usize,
}

impl<T: Copy + Default> FixedRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(2 * capacity),
            cap: capacity,
            head: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.buf.len() - self.head
    }

    pub fn push(&mut self, elem: T) {
        // shift left if buffer is full, use pointer to memmove
        // already reduced memory allocation with larger buffer
        if self.buf.len() == self.buf.capacity() {
            let len = self.buf.len();
            self.buf.copy_within(self.head + 1..len, 0);
            self.buf.resize(self.cap - 1, Default::default());
            self.head = 0;
        }
        self.buf.push(elem);
        if self.buf.len() > self.cap {
            self.head += 1;
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.buf[self.head..self.buf.len()]
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.as_slice().iter()
    }
}

#[cfg(test)]
mod test {
    use super::FixedRingBuffer;

    #[test]
    pub fn test_basic_push() {
        let mut ringbuffer = FixedRingBuffer::new(3);
        let expect = vec![
            vec![0],
            vec![0, 1],
            vec![0, 1, 2],
            vec![1, 2, 3],
            vec![2, 3, 4],
            vec![3, 4, 5],
            vec![4, 5, 6],
            vec![5, 6, 7],
            vec![6, 7, 8],
            vec![7, 8, 9],
        ];
        for (x, expect) in (0..10).zip(expect.iter()) {
            ringbuffer.push(x);
            assert_eq!(ringbuffer.as_slice(), expect.as_slice());
        }
    }
}
