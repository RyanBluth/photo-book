
pub fn partition_iterator<T>(iter: impl Iterator<Item = T>, partitions: usize) -> Vec<Vec<T>> {
    let mut output: Vec<Vec<T>> = (0..partitions).map(|_| Vec::new()).collect();
    for (i, item) in iter.enumerate() {
        let partition_index = i % partitions;
        output[partition_index].push(item);
    }
    output
}
