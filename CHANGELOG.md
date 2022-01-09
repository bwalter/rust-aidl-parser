# Change Log

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.9.1] - 2022-01-10

- Fix resolving of declared parcelables

## [0.9.0] - 2022-01-10

- Full support for declared parcelables
- Improve representation of resolved types in the AST
- Add support for builtin Android types (ParcelableHolder, IBinder, ...)
- Rename Member into Field
- Add Symbol::get_qualified_name()
- Improve range info for generic types
- Make a few errors more descriptive

## [0.8.0] - 2022-01-04

- Allow AST types to be deserialized and adjust case

## [0.7.3] - 2022-01-04

- Fixed direction validation for interface method arguments

## [0.7.2] - 2022-01-04

- Fixed minor issues when traversing arrays
- Updated dependencies

## [0.7.1] - 2022-01-03

- Fixed minor issues when traversing arrays

## [0.7.0] - 2022-01-03

- Added support for oneway interfaces
- Initial support for declared parcelables

## [0.6.2] - 2022-01-02

- Fixed release automation issues

## [0.6.1] - 2022-01-02

- Fixed unit test issues

## [0.6.0] - 2022-01-01

- Added traverse::filter_symbols() and small fixes

## [0.5.0] - 2022-01-01

- Check for duplicated method name
- Check for duplicated method id
- Check for mixed usage of methods with/without id
- Introduce traverse::find_symbol() and traverse::find_symbol_by_position()

## [0.4.0] - 2021-12-30

- Improved documentation
- New public module traverse to help visiting AST
- Refactor validation
- Various grammar improvements

## [0.3.0] - 2021-12-24

- Add support for non-generic lists/maps
- Better check for method number
- Add support for object values

## [0.2.0] - 2021-12-23

- Fix wrong types for maps
- Validate method arg direction based on type
- General validation and diagnostics improvements

## [0.1.5] - 2021-12-22

- Initial version

