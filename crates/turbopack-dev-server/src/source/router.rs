use anyhow::Result;
use turbo_tasks::{TryJoinIterExt, Value, Vc};
use turbopack_core::introspect::{Introspectable, IntrospectableChildren};

use super::{ContentSource, ContentSourceData, ContentSourceResult};
use crate::source::ContentSources;

/// Binds different ContentSources to different subpaths. A fallback
/// ContentSource will serve all other subpaths.
#[turbo_tasks::value(shared)]
pub struct RouterContentSource {
    pub routes: Vec<(String, Vc<Box<dyn ContentSource>>)>,
    pub fallback: Vc<Box<dyn ContentSource>>,
}

impl RouterContentSource {
    fn get_source<'s, 'a>(&'s self, path: &'a str) -> (&'s Vc<Box<dyn ContentSource>>, &'a str) {
        for (route, source) in self.routes.iter() {
            if path.starts_with(route) {
                let path = &path[route.len()..];
                return (source, path);
            }
        }
        (&self.fallback, path)
    }
}

#[turbo_tasks::value_impl]
impl ContentSource for RouterContentSource {
    #[turbo_tasks::function]
    async fn get(
        &self,
        path: String,
        data: Value<ContentSourceData>,
    ) -> Result<Vc<ContentSourceResult>> {
        let (source, path) = self.get_source(path);
        Ok(source.resolve().await?.get(path, data))
    }

    #[turbo_tasks::function]
    fn get_children(&self) -> Vc<ContentSources> {
        let mut sources = Vec::with_capacity(self.routes.len() + 1);

        sources.extend(self.routes.iter().map(|r| r.1));
        sources.push(self.fallback);

        Vc::cell(sources)
    }
}

#[turbo_tasks::function]
fn introspectable_type() -> Vc<String> {
    Vc::cell("router content source".to_string())
}

#[turbo_tasks::value_impl]
impl Introspectable for RouterContentSource {
    #[turbo_tasks::function]
    fn ty(&self) -> Vc<String> {
        introspectable_type()
    }

    #[turbo_tasks::function]
    async fn children(&self) -> Result<Vc<IntrospectableChildren>> {
        Ok(Vc::cell(
            self.routes
                .iter()
                .cloned()
                .chain(std::iter::once((String::new(), self.fallback)))
                .map(|(path, source)| (Vc::cell(path), source))
                .map(|(path, source)| async move {
                    Ok(Vc::try_resolve_sidecast::<Box<dyn Introspectable>>(source)
                        .await?
                        .map(|i| (path, i)))
                })
                .try_join()
                .await?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }
}
