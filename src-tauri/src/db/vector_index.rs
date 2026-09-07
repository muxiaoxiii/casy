//! Rebuildable Zvec cache. SQLite remains the source of truth for versions and passages.
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use zvec_rust::{
    Collection, CollectionOptions, CollectionSchema, ConfigBuilder, DataType, Doc, HnswQueryParams,
    IndexParams, MetricType, SearchQuery,
};

use crate::ai::embeddings::stored_cosine;

static INITIALIZED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
static CACHE: Mutex<Option<VectorIndex>> = Mutex::new(None);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Job {
    revision: String,
    count: usize,
}
#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    fingerprint: String,
    dimension: usize,
    revision: String,
    jobs: BTreeMap<String, Job>,
}

pub struct VectorIndex {
    collection: Option<Collection>,
    root: PathBuf,
    manifest: Manifest,
    job_lookup: HashMap<String, String>,
    _lock: fs::File,
}

fn initialize() -> Result<()> {
    INITIALIZED
        .get_or_init(|| {
            zvec_rust::initialize(Some(
                &ConfigBuilder::new()
                    .num_threads(2)
                    .memory_limit(512 * 1024 * 1024)
                    .enable_console_log(false),
            ))
            .map_err(|e| e.to_string())
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

fn fp16(values: &[f32]) -> Vec<u16> {
    values
        .iter()
        .map(|&v| half::f16::from_f32(v).to_bits())
        .collect()
}

fn normalized(bytes: &[u8], dimension: usize) -> Result<Vec<f32>> {
    let mut values: Vec<f32> = if bytes.len() == dimension + 4 && bytes.starts_with(b"CVQ1") {
        bytes[4..].iter().map(|&v| v as i8 as f32).collect()
    } else {
        ensure!(bytes.len() == dimension * 4, "向量数据维数无效，请重建索引");
        bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect()
    };
    let norm = values
        .iter()
        .map(|&v| (v as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    ensure!(norm.is_finite() && norm > 0.0, "向量数据无效，请重建索引");
    for v in &mut values {
        *v = (*v as f64 / norm) as f32;
    }
    Ok(values)
}

fn check_ffi(code: u32) -> Result<()> {
    if code == 0 {
        return Ok(());
    }
    // The C API owns the error buffer; copy it before releasing it with its allocator.
    let message = unsafe {
        let mut pointer = std::ptr::null_mut();
        zvec_rust_sys::zvec_get_last_error(&mut pointer);
        if pointer.is_null() {
            format!("Zvec error {code}")
        } else {
            let text = std::ffi::CStr::from_ptr(pointer)
                .to_string_lossy()
                .into_owned();
            zvec_rust_sys::zvec_free(pointer.cast());
            text
        }
    };
    bail!("{message}")
}

impl VectorIndex {
    pub fn open(root: &Path) -> Result<Self> {
        initialize()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(root)?;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(root)?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("lock"))?;
        lock.try_lock()
            .context("向量索引正被另一 Casy 进程使用，本次返回关键词结果")?;
        let manifest: Manifest = fs::read(root.join("manifest.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let job_lookup = manifest
            .jobs
            .keys()
            .map(|id| (key(id), id.clone()))
            .collect();
        Ok(Self {
            collection: None,
            root: root.into(),
            manifest,
            job_lookup,
            _lock: lock,
        })
    }

    fn options() -> Result<CollectionOptions> {
        let mut options = CollectionOptions::new()?;
        options.set_enable_mmap(true)?;
        options.set_max_buffer_size(8 * 1024 * 1024)?;
        Ok(options)
    }

    fn rebuild(&mut self, fingerprint: &str, dimension: usize) -> Result<()> {
        self.collection = None;
        let path = self.root.join("collection");
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        let schema = CollectionSchema::builder("knowledge")
            .add_indexed_field("item", DataType::String, IndexParams::invert(false, false)?)
            .add_vector_field(
                "embedding",
                DataType::VectorFp16,
                dimension as u32,
                IndexParams::hnsw(MetricType::Cosine, 16, 200)?,
            )
            .build()?;
        self.collection = Some(Collection::create_and_open(
            path.to_str().context("索引路径编码无效")?,
            &schema,
            Some(&Self::options()?),
        )?);
        self.manifest = Manifest {
            fingerprint: fingerprint.into(),
            dimension,
            ..Default::default()
        };
        self.job_lookup.clear();
        Ok(())
    }

    fn sync(&mut self, conn: &Connection, fingerprint: &str, dimension: usize) -> Result<()> {
        ensure!((1..=8192).contains(&dimension), "向量维数无效");
        let revision: String = conn.query_row(
            "SELECT revision FROM knowledge_ann_state WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        let path = self.root.join("collection");
        if self.manifest.fingerprint != fingerprint
            || self.manifest.dimension != dimension
            || !path.exists()
            || self.manifest.revision.is_empty()
        {
            self.rebuild(fingerprint, dimension)?;
        } else if self.collection.is_none() {
            let opened = Collection::open(
                path.to_str().context("索引路径编码无效")?,
                Some(&Self::options()?),
            );
            match opened {
                Ok(collection)
                    if collection.stats().is_ok_and(|s| {
                        s.doc_count
                            == self
                                .manifest
                                .jobs
                                .values()
                                .map(|j| j.count as u64)
                                .sum::<u64>()
                    }) =>
                {
                    self.collection = Some(collection)
                }
                _ => self.rebuild(fingerprint, dimension)?,
            }
        }
        if self.manifest.revision == revision {
            return Ok(());
        }
        let jobs: BTreeMap<String, Job> = {
            let mut stmt = conn.prepare(
                "SELECT j.id,a.revision,j.total_chunks FROM knowledge_index_jobs j
                JOIN knowledge_ann_jobs a ON a.job_id=j.id JOIN knowledge_items k ON k.id=j.item_id
                WHERE j.status='completed' AND j.config_hash=?1 AND j.dimension=?2
                AND COALESCE(k.status,'current')='current'",
            )?;
            let result = stmt
                .query_map(params![fingerprint, dimension], |r| {
                    Ok((
                        r.get(0)?,
                        Job {
                            revision: r.get(1)?,
                            count: r.get(2)?,
                        },
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?;
            result
        };
        // An interrupted update leaves no valid manifest, forcing recovery from SQLite.
        let manifest_path = self.root.join("manifest.json");
        if manifest_path.exists() {
            fs::remove_file(&manifest_path)?;
        }
        self.manifest.revision.clear();
        let collection = self.collection.as_ref().unwrap();
        let mut changed = false;
        for (id, old) in &self.manifest.jobs {
            if jobs.get(id) == Some(old) {
                continue;
            }
            changed = true;
            for start in (0..old.count).step_by(256) {
                let pks: Vec<_> = (start..(start + 256).min(old.count))
                    .map(|n| format!("{}_{n}", key(id)))
                    .collect();
                let result =
                    collection.delete(&pks.iter().map(String::as_str).collect::<Vec<_>>())?;
                ensure!(result.error_count == 0, "Zvec 删除分段失败");
            }
        }
        for (id, job) in &jobs {
            if self.manifest.jobs.get(id) == Some(job) {
                continue;
            }
            changed = true;
            let mut stmt = conn.prepare("SELECT j.item_id,c.chunk_index,c.embedding FROM knowledge_index_chunks c
                JOIN knowledge_index_jobs j ON j.id=c.job_id WHERE c.job_id=?1 ORDER BY c.chunk_index")?;
            let mut rows = stmt.query([id])?;
            let mut batch = Vec::new();
            let mut count = 0;
            while let Some(row) = rows.next()? {
                let item: String = row.get(0)?;
                let index: usize = row.get(1)?;
                ensure!(
                    index == count && !id.contains('\0'),
                    "向量分段不完整，请重建索引"
                );
                let bytes: Vec<u8> = row.get(2)?;
                let values = fp16(&normalized(&bytes, dimension)?);
                let mut doc = Doc::new()?;
                doc.set_pk(&format!("{}_{index}", key(id)));
                doc.add_string("item", &key(&item))?;
                // zvec-rust lacks an FP16 setter; the C API copies this typed byte slice.
                check_ffi(unsafe {
                    zvec_rust_sys::zvec_doc_add_field_by_value(
                        doc.as_raw(),
                        c"embedding".as_ptr(),
                        DataType::VectorFp16 as u32,
                        values.as_ptr().cast(),
                        values.len() * 2,
                    )
                })?;
                batch.push(doc);
                count += 1;
                if batch.len() == 256 {
                    insert(collection, &batch)?;
                    batch.clear();
                }
            }
            if !batch.is_empty() {
                insert(collection, &batch)?;
            }
            ensure!(count == job.count && count > 0, "向量分段缺失，请重建索引");
        }
        if changed {
            collection.flush()?;
            collection.optimize()?;
        }
        self.manifest.jobs = jobs;
        self.job_lookup = self
            .manifest
            .jobs
            .keys()
            .map(|id| (key(id), id.clone()))
            .collect();
        self.manifest.revision = revision;
        let mut ready = tempfile::NamedTempFile::new_in(&self.root)?;
        serde_json::to_writer(&mut ready, &self.manifest)?;
        ready.flush()?;
        ready.as_file().sync_all()?;
        ready.persist(manifest_path)?;
        Ok(())
    }

    pub fn search(
        &mut self,
        conn: &Connection,
        query: &[f32],
        fingerprint: &str,
        limit: usize,
    ) -> Result<Vec<(String, f64, String)>> {
        ensure!(
            !query.is_empty() && query.iter().all(|v| v.is_finite()),
            "检索向量无效"
        );
        self.sync(conn, fingerprint, query.len())?;
        let collection = self.collection.as_ref().unwrap();
        if self.manifest.jobs.is_empty() {
            return Ok(vec![]);
        }
        let query = normalized(
            &query
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
            query.len(),
        )?;
        let values = fp16(&query);
        let mut best = HashMap::<String, (f64, String)>::new();
        let mut excluded = Vec::<String>::new();
        let limit = limit.clamp(1, 100);
        // Exclude already covered notes so one long document cannot monopolize every candidate.
        for _ in 0..4 {
            let mut request =
                SearchQuery::new("embedding", &query, (limit * 8).clamp(64, 512) as i32)?;
            check_ffi(unsafe {
                zvec_rust_sys::zvec_vector_query_set_query_vector(
                    request.as_raw(),
                    values.as_ptr().cast(),
                    values.len() * 2,
                )
            })?;
            request.set_hnsw_params(HnswQueryParams::new(256, 0.0, false, false))?;
            request.set_include_vector(false)?;
            if !excluded.is_empty() {
                request.set_filter(&format!(
                    "item NOT IN ({})",
                    excluded
                        .iter()
                        .map(|s| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")))
                        .collect::<Vec<_>>()
                        .join(",")
                ))?;
            }
            let candidates = collection.query(&request)?;
            if candidates.is_empty() {
                break;
            }
            let before = excluded.len();
            for doc in candidates {
                let Some((job, index)) = doc.get_pk().and_then(|pk| pk.rsplit_once('_')) else {
                    continue;
                };
                let Some(job) = self.job_lookup.get(job) else {
                    continue;
                };
                let Ok(index) = index.parse::<usize>() else {
                    continue;
                };
                let row: Option<(String,Vec<u8>,String)> = conn.query_row("SELECT j.item_id,c.embedding,c.content
                    FROM knowledge_index_chunks c JOIN knowledge_index_jobs j ON j.id=c.job_id
                    JOIN knowledge_items k ON k.id=j.item_id WHERE c.job_id=?1 AND c.chunk_index=?2
                    AND j.status='completed' AND j.config_hash=?3 AND COALESCE(k.status,'current')='current'",
                    params![job,index,fingerprint], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
                if let Some((item, bytes, content)) = row {
                    let item_key = key(&item);
                    if !excluded.contains(&item_key) {
                        excluded.push(item_key);
                    }
                    let score = stored_cosine(&query, &bytes);
                    if score > 0.1 && best.get(&item).is_none_or(|(old, _)| score > *old) {
                        best.insert(item, (score, content));
                    }
                }
            }
            if best.len() >= limit || excluded.len() == before {
                break;
            }
        }
        let mut hits: Vec<_> = best
            .into_iter()
            .map(|(id, (score, text))| (id, score, text))
            .collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        hits.truncate(limit);
        Ok(hits)
    }
}

fn key(id: &str) -> String {
    crate::ai::embeddings::digest(id.as_bytes())[..32].to_owned()
}

fn insert(collection: &Collection, batch: &[Doc]) -> Result<()> {
    let result = collection.insert(&batch.iter().collect::<Vec<_>>())?;
    ensure!(
        result.error_count == 0 && result.success_count == batch.len() as u64,
        "Zvec 写入分段失败"
    );
    Ok(())
}

pub fn search(
    conn: &Connection,
    query: &[f32],
    fingerprint: &str,
    limit: usize,
) -> Result<Vec<(String, f64, String)>> {
    let Some(path) = conn.path().filter(|s| !s.is_empty()) else {
        let root = tempfile::tempdir()?;
        return VectorIndex::open(root.path())?.search(conn, query, fingerprint, limit);
    };
    let root = Path::new(path)
        .parent()
        .context("数据库目录无效")?
        .join("indexes/zvec-v1");
    let mut cache = CACHE
        .lock()
        .map_err(|_| anyhow::anyhow!("向量索引锁不可用"))?;
    if cache.as_ref().is_none_or(|index| index.root != root) {
        *cache = None;
        *cache = Some(VectorIndex::open(&root)?);
    }
    let result = cache
        .as_mut()
        .unwrap()
        .search(conn, query, fingerprint, limit);
    if result.is_err() {
        *cache = None;
    }
    result
}

pub fn prepare() -> Result<()> {
    let mut connection = super::open_db()?;
    let conn = connection.transaction()?;
    let Some(plan) = crate::ai::embeddings::EmbeddingPlan::load(&conn)? else {
        return Ok(());
    };
    let dimension: Option<usize> = conn
        .query_row(
            "SELECT dimension FROM knowledge_index_jobs
        WHERE status='completed' AND config_hash=?1 LIMIT 1",
            [&plan.fingerprint],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(dimension) = dimension {
        // Synchronize without computing a query embedding or consuming inference capacity.
        let root = super::get_db_path()
            .parent()
            .context("数据库目录无效")?
            .join("indexes/zvec-v1");
        let mut cache = CACHE
            .lock()
            .map_err(|_| anyhow::anyhow!("向量索引锁不可用"))?;
        if cache.as_ref().is_none_or(|index| index.root != root) {
            *cache = None;
            *cache = Some(VectorIndex::open(&root)?);
        }
        if let Err(error) = cache
            .as_mut()
            .unwrap()
            .sync(&conn, &plan.fingerprint, dimension)
        {
            *cache = None;
            return Err(error);
        }
    } else {
        let root = super::get_db_path()
            .parent()
            .context("数据库目录无效")?
            .join("indexes/zvec-v1");
        if root.join("collection").exists() {
            let mut cache = CACHE
                .lock()
                .map_err(|_| anyhow::anyhow!("向量索引锁不可用"))?;
            *cache = None;
            let _owned = VectorIndex::open(&root)?;
            fs::remove_dir_all(root.join("collection"))?;
            let manifest = root.join("manifest.json");
            if manifest.exists() {
                fs::remove_file(manifest)?;
            }
        }
    }
    Ok(())
}

/// Isolated installer smoke check; never opens an application profile.
pub fn probe() -> Result<serde_json::Value> {
    let root = tempfile::tempdir()?;
    let conn = Connection::open_in_memory()?;
    super::schema::run_migrations(&conn, 0)?;
    conn.execute_batch("INSERT INTO knowledge_items(id,title,category,content) VALUES('probe','probe','note','probe');
        INSERT INTO knowledge_index_jobs(id,item_id,source_hash,config_hash,model,status,total_chunks,completed_chunks,dimension)
        VALUES('probe','probe','probe','probe','probe','completed',1,1,4);")?;
    conn.execute("INSERT INTO knowledge_index_chunks(job_id,chunk_index,content,embedding) VALUES('probe',0,'probe',?1)",
        [crate::ai::embeddings::compact_vector(&[1.,0.,0.,0.])])?;
    let mut index = VectorIndex::open(root.path())?;
    let results = index.search(&conn, &[1., 0., 0., 0.], "probe", 1)?;
    ensure!(
        results.len() == 1 && results[0].0 == "probe" && results[0].1 > 0.99,
        "Zvec 自检失败"
    );
    Ok(
        serde_json::json!({"available":true,"engine":"zvec","index":"hnsw-fp16","version":zvec_rust::version()}),
    )
}
