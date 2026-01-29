# [0.7.0](https://github.com/debba/tabularis/compare/v0.6.1...v0.7.0) (2026-01-29)


### Features

* **data-grid:** improve table extraction and cell rendering ([fd21915](https://github.com/debba/tabularis/commit/fd21915983ddfb85b40a4d432c4cccea8c551ee0))
* **drivers:** enhance multi-database decimal and null value handling ([4d49f66](https://github.com/debba/tabularis/commit/4d49f66eb407f8b9b59d11efc645655d16bf7a95))
* **drivers:** improve datetime parsing and formatting ([74c394b](https://github.com/debba/tabularis/commit/74c394b8ae1852bba70f60bbdee7665d1b066b99))
* **editor:** improve query execution loading state ([d1decc1](https://github.com/debba/tabularis/commit/d1decc1f46d79bc4b557c8b80d10191890e2610a))
* **settings:** fix external links by using opener plugin ([11acdb5](https://github.com/debba/tabularis/commit/11acdb520aa7e93f9eb04f8f824e6c0e3a87ceeb))
* **ui:** implement batch editing with pending changes and deletions ([cb6aecb](https://github.com/debba/tabularis/commit/cb6aecb319a857d7e300bd50f378ffa2bdd9472d))
* **website:** add landing page and sync version handling ([471bf68](https://github.com/debba/tabularis/commit/471bf682ac06a0882a26f296b2e4101bf45c1b18))



## [0.6.1](https://github.com/debba/debba.sql/compare/v0.6.0...v0.6.1) (2026-01-28)


### Features

* **version:** add APP_VERSION export and sync script ([54aeaa6](https://github.com/debba/debba.sql/commit/54aeaa6274cc9e906b016b24ffd91ef38881e129))



# [0.6.0](https://github.com/debba/debba.sql/compare/v0.5.0...v0.6.0) (2026-01-28)


### Features

* **i18n:** add internationalization support and bump version to 0.6.0 ([e1cab12](https://github.com/debba/debba.sql/commit/e1cab1255165c8133d929cc075c08900fc7a3067))
* **security:** integrate system keychain for connection passwords ([ab284b5](https://github.com/debba/debba.sql/commit/ab284b52d7fc204c4551ec66c5cd8c34c404ca81))
* **window:** add Wayland window title workaround for Linux ([c09ae72](https://github.com/debba/debba.sql/commit/c09ae7261ed88f3924a84e3e8b00f470176f07af))



# [0.5.0](https://github.com/debba/debba.sql/compare/v0.4.0...v0.5.0) (2026-01-27)


### Bug Fixes

* restore pagination controls and fix truncated flag scope ([1bdf104](https://github.com/debba/debba.sql/commit/1bdf104c37c057f183ed9f37f97abd40b31fbd66))


### Features

* release v0.5.0 - Advanced Schema Management & UX Improvements ([f2d7d1c](https://github.com/debba/debba.sql/commit/f2d7d1c841ef6a0d62b22e8ec27bef8ef845113e))
* **schema:** add foreign key, index structs and: column edit UI ([c20c550](https://github.com/debba/debba.sql/commit/c20c550c3661bcc8dd0dbb09e02149fdf92ccaef))
* **sidebar:** add column explorer with delete action ([b25cd50](https://github.com/debba/debba.sql/commit/b25cd508aef8d58f0894976068d9ee5621f69e9a))
* **ui:** add multi-row selection and select-all column to DataGrid ([66ddfaa](https://github.com/debba/debba.sql/commit/66ddfaa86c01dc73c452bb04d2608cfdc640c07a))



# [0.4.0](https://github.com/debba/debba.sql/compare/v0.3.0...v0.4.0) (2026-01-27)


### Features

* **ci:** add readme downloads workflow ([d48ef6b](https://github.com/debba/debba.sql/commit/d48ef6bb77e9a654b8081080eb0f40756dcef280))
* **editor:** add DataGrip-style multiple query tabs with isolation ([688739a](https://github.com/debba/debba.sql/commit/688739aac8eb995e1329943ef43e290d8b503f8d))
* **visual-query-builder:** add delete table node UI and auto GROUP BY ([0f1f9be](https://github.com/debba/debba.sql/commit/0f1f9bebd9143f9d155c0790628acf199cd79e24))
* **visual-query-builder:** add visual query builder UI ([f97b67a](https://github.com/debba/debba.sql/commit/f97b67a459dd3d7e4465622c2702bbfdd1439e99))



# [0.3.0](https://github.com/debba/debba.sql/compare/v0.2.0...v0.3.0) (2026-01-27)


### Features

* **connection:** add duplicate connection command and clone button ([4e00382](https://github.com/debba/debba.sql/commit/4e003828a491c18a2d348a6efcc86ccfffcadcc2))



# [0.2.0](https://github.com/debba/debba.sql/compare/3a9fc495d44cdd907d5f561a73d5734d0ccb0590...v0.2.0) (2026-01-27)


### Bug Fixes

* **drivers:** support additional numeric types and correct row mapping ([0769f3b](https://github.com/debba/debba.sql/commit/0769f3b4ed38fe2a531ff9ac7b6affed70af75b2))


### Features

* add query cancellation, sanitization, and multi‑statement support ([403956a](https://github.com/debba/debba.sql/commit/403956ab596a3808d9fcb65358bcbaf857cba1ed))
* **connections:** add error handling UI and propagate connection errors ([3494021](https://github.com/debba/debba.sql/commit/34940210025808434ea7c333263714792ae03b02))
* **editor:** add run dropdown and dynamic window title ([99b3d1c](https://github.com/debba/debba.sql/commit/99b3d1c3fba7b424533a4ebad4629d5bec1c5484))
* **pagination:** implement server‑side pagination and UI controls ([f50b110](https://github.com/debba/debba.sql/commit/f50b11001ac1eb82d310fcb23bc51c50881a9b52))
* **saved-queries:** add saved queries support ([9839737](https://github.com/debba/debba.sql/commit/9839737fc2d532e4e139226fc5e331f722ba57de))
* **settings:** implement query limit UI and backend streaming support ([9fd89f3](https://github.com/debba/debba.sql/commit/9fd89f3c3b3538b0d09fe8324e89ba4172339100))
* **ssh:** add SSH tunnel support with connection edit/delete UI ([3a9fc49](https://github.com/debba/debba.sql/commit/3a9fc495d44cdd907d5f561a73d5734d0ccb0590))
* **ssh:** add system SSH backend and URL encoding for DB URLs ([5e93ea3](https://github.com/debba/debba.sql/commit/5e93ea38f1a74966ab1a41f5ddda4e8cb13bb23c))



