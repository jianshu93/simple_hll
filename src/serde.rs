use crate::{Hasher, HyperLogLog};

#[derive(serde::Serialize, borsh::BorshSerialize)]
enum HyperLogLogVariantRef<'a> {
    Empty,
    Sparse { data: Vec<(u16, u8)> },
    Full(&'a Vec<u8>),
}

#[derive(serde::Deserialize, borsh::BorshDeserialize)]
enum HyperLogLogVariant {
    Empty,
    Sparse { data: Vec<(u16, u8)> },
    Full(Vec<u8>),
}

impl<H: Hasher + Default, const P: usize> From<HyperLogLogVariant> for HyperLogLog<H, P> {
    fn from(value: HyperLogLogVariant) -> Self {
        match value {
            HyperLogLogVariant::Empty => HyperLogLog::<H, P>::new(),
            HyperLogLogVariant::Sparse { data } => {
                let mut registers = vec![0; 1 << P];
                for (index, val) in data {
                    registers[index as usize] = val;
                }

                HyperLogLog::<H, P>::with_registers(registers)
            }
            HyperLogLogVariant::Full(registers) => HyperLogLog::<H, P>::with_registers(registers),
        }
    }
}

impl<'a, H: Hasher + Default, const P: usize> From<&'a HyperLogLog<H, P>>
    for HyperLogLogVariantRef<'a>
{
    fn from(hll: &'a HyperLogLog<H, P>) -> Self {
        let none_empty_registers =
            HyperLogLog::<H, P>::number_registers() - hll.num_empty_registers();

        if none_empty_registers == 0 {
            HyperLogLogVariantRef::Empty
        } else if none_empty_registers * 3 <= HyperLogLog::<H, P>::number_registers() {
            // If the number of empty registers is larger enough, we can use sparse serialize to reduce the binary size
            // each register in sparse format will occupy 3 bytes, 2 for register index and 1 for register value.
            let sparse_data: Vec<(u16, u8)> = hll
                .registers
                .iter()
                .enumerate()
                .filter_map(|(index, &value)| {
                    if value != 0 {
                        Some((index as u16, value))
                    } else {
                        None
                    }
                })
                .collect();

            HyperLogLogVariantRef::Sparse { data: sparse_data }
        } else {
            HyperLogLogVariantRef::Full(&hll.registers)
        }
    }
}

impl<H: Hasher + Default, const P: usize> serde::Serialize for HyperLogLog<H, P> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v: HyperLogLogVariantRef<'_> = self.into();
        v.serialize(serializer)
    }
}

impl<'de, H: Hasher + Default, const P: usize> serde::Deserialize<'de> for HyperLogLog<H, P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = HyperLogLogVariant::deserialize(deserializer)?;
        Ok(v.into())
    }
}

impl<H: Hasher + Default, const P: usize> borsh::BorshSerialize for HyperLogLog<H, P> {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let v: HyperLogLogVariantRef<'_> = self.into();
        v.serialize(writer)
    }
}

impl<H: Hasher + Default, const P: usize> borsh::BorshDeserialize for HyperLogLog<H, P> {
    fn deserialize_reader<R: std::io::prelude::Read>(reader: &mut R) -> std::io::Result<Self> {
        let v = HyperLogLogVariant::deserialize_reader(reader)?;
        Ok(v.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::HyperLogLog;

    const P: usize = 14;
    type H = ahash::AHasher;

    #[test]
    fn test_serde() {
        let mut hll = HyperLogLog::<H, P>::new();
        json_serde_equal(&hll);

        for i in 0..100000 {
            hll.add_object(&(i % 200));
        }
        json_serde_equal(&hll);

        let hll = HyperLogLog::<H, P>::with_registers(vec![1; 1 << P]);
        json_serde_equal(&hll);
    }

    fn json_serde_equal<T>(t: &T)
    where
        T: serde::Serialize + for<'a> serde::Deserialize<'a> + Eq,
    {
        let val = serde_json::to_vec(t).unwrap();
        let new_t: T = serde_json::from_slice(&val).unwrap();
        assert!(t == &new_t)
    }
}
