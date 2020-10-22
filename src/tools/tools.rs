use std::io::Result;
use rand::{SeedableRng, StdRng, Rng};
/// 根据一个种子值获取原data中的随机num个元素
/// data.len() 必须 > num
/// 返回一个新的vec
pub fn get_random_by_seed(seed: u32, data: &Vec<u32>, num: usize) -> Box<Vec<u32>> {
    let data_len = data.len();
    if data_len < num {
        panic!("num 必须小于 data集合元素的个数");
    }
    let mut seed_rng = StdRng::seed_from_u64(num as u64);
    // index_ck 用来检查已经出现的索引值
    let mut index_ck: Vec<u32> = Vec::new();
    let size = ori_data.len();
    // 存放新的集合
    let mut new_list: Vec<u32> = Vec::new();
    while index_ck.len() < num {
        let temp: f32 = seed_rng.gen();
        // 取到目标索引值
        let v:u32 = (temp * size as f32) as u32;
        // 如果查找vec，不存在v值则添加进new_list中
        if !index_ck.binary_search(&v).is_ok() {
            // index_ck 中 插入一个值
            index_ck.push(v);
            match ori_data.get(v  as usize) {
                Some(result) =>  {
                    new_list.push(*result);
                },
                None => panic!("没有对应的索引")
            };
        }
    }
    Box::from(new_list)
}

