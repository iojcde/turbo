use anyhow::Result;
use turbo_tasks::{Value, Vc};
use turbo_tasks_fs::{DirectoryContent, DirectoryEntry, FileSystemEntryType, FileSystemPath};
use turbopack_core::{
    asset::Asset,
    introspect::{asset::IntrospectableAsset, Introspectable, IntrospectableChildren},
    source_asset::SourceAsset,
    version::VersionedContentExt,
};

use super::{ContentSource, ContentSourceContent, ContentSourceData, ContentSourceResult};

#[turbo_tasks::value(shared)]
pub struct StaticAssetsContentSource {
    pub prefix: String,
    pub dir: Vc<FileSystemPath>,
}

#[turbo_tasks::value_impl]
impl StaticAssetsContentSource {
    #[turbo_tasks::function]
    pub fn new(prefix: String, dir: Vc<FileSystemPath>) -> Vc<StaticAssetsContentSource> {
        let mut prefix = prefix;
        if !prefix.is_empty() && !prefix.ends_with('/') {
            prefix.push('/');
        }
        StaticAssetsContentSource { prefix, dir }.cell()
    }
}

#[turbo_tasks::value_impl]
impl ContentSource for StaticAssetsContentSource {
    #[turbo_tasks::function]
    async fn get(
        &self,
        path: String,
        _data: Value<ContentSourceData>,
    ) -> Result<Vc<ContentSourceResult>> {
        if !path.is_empty() {
            if let Some(path) = path.strip_prefix(&self.prefix) {
                let path = self.dir.join(path.to_string());
                let ty = path.get_type().await?;
                if matches!(
                    &*ty,
                    FileSystemEntryType::File | FileSystemEntryType::Symlink
                ) {
                    let content = Vc::upcast::<Box<dyn Asset>>(SourceAsset::new(path)).content();
                    return Ok(ContentSourceResult::exact(Vc::upcast(
                        ContentSourceContent::static_content(content.versioned()),
                    )));
                }
            }
        }
        Ok(ContentSourceResult::not_found())
    }
}

#[turbo_tasks::value_impl]
impl Introspectable for StaticAssetsContentSource {
    #[turbo_tasks::function]
    fn ty(&self) -> Vc<String> {
        Vc::cell("static assets directory content source".to_string())
    }

    #[turbo_tasks::function]
    async fn children(&self) -> Result<Vc<IntrospectableChildren>> {
        let dir = self.dir.read_dir().await?;
        let children = match &*dir {
            DirectoryContent::NotFound => Default::default(),
            DirectoryContent::Entries(entries) => entries
                .iter()
                .map(|(name, entry)| {
                    let child = match entry {
                        DirectoryEntry::File(path) | DirectoryEntry::Symlink(path) => {
                            IntrospectableAsset::new(Vc::upcast(SourceAsset::new(*path)))
                        }
                        DirectoryEntry::Directory(path) => {
                            Vc::upcast(StaticAssetsContentSource::new(
                                format!("{prefix}{name}", prefix = self.prefix),
                                *path,
                            ))
                        }
                        DirectoryEntry::Other(_) => todo!("what's DirectoryContent::Other?"),
                        DirectoryEntry::Error => todo!(),
                    };
                    (Vc::cell(name.clone()), child)
                })
                .collect(),
        };
        Ok(Vc::cell(children))
    }
}
