use crate::typescript::convert_type;
use crate::{utils, BuildState};
use convert_case::{Case, Casing};

impl super::ToTypescript for syn::ItemStruct {
    fn convert_to_ts(self, state: &mut BuildState, config: &crate::BuildSettings) {
        let export = if config.uses_type_interface { "" } else { "export " };
        let casing = utils::get_attribute_arg("serde", "rename_all", &self.attrs);
        let casing = utils::parse_serde_case(casing);
        state.types.push('\n');

        let comments = utils::get_comments(self.clone().attrs);
        state.write_comments(&comments, 0);

        let intersections = get_intersections(&self.fields);

        match intersections {
            Some(intersections) => {
                state.types.push_str(&format!(
                    "{export}type {struct_name}{generics} = {intersections} & {{\n",
                    export = export,
                    struct_name = self.ident,
                    generics = utils::extract_struct_generics(self.generics.clone()),
                    intersections = intersections
                ));
            }
            None => {
                state.types.push_str(&format!(
                    "{export}interface {interface_name}{generics} {{\n",
                    interface_name = self.ident,
                    generics = utils::extract_struct_generics(self.generics.clone())
                ));
            }
        }

        process_fields(self.fields, state, 2, casing);
        state.types.push('}');
        state.types.push('\n');
    }
}

pub fn process_fields(
    fields: syn::Fields,
    state: &mut BuildState,
    indentation_amount: i8,
    case: impl Into<Option<Case>>,
) {
    let space = utils::build_indentation(indentation_amount);
    let case = case.into();
    for field in fields {
        // Check if the field has the serde flatten attribute, if so, skip it
        let has_flatten_attr = utils::get_attribute_arg("serde", "flatten", &field.attrs).is_some();
        if has_flatten_attr {
            continue;
        }

        let comments = utils::get_comments(field.attrs);

        state.write_comments(&comments, 2);
        let field_name = if let Some(name_case) = case {
            field
                .ident
                .map(|id| id.to_string().to_case(name_case))
                .unwrap()
        } else {
            field.ident.map(|i| i.to_string()).unwrap()
        };

        let field_type = convert_type(&field.ty);
        state.types.push_str(&format!(
            "{space}{field_name}{optional_parameter_token}: {field_type};\n",
            space = space,
            field_name = field_name,
            optional_parameter_token = if field_type.is_optional { "?" } else { "" },
            field_type = field_type.ts_type
        ));
    }
}

fn get_intersections(fields: &syn::Fields) -> Option<String> {
    let mut types = Vec::new();

    for field in fields {
        let has_flatten_attr = utils::get_attribute_arg("serde", "flatten", &field.attrs).is_some();
        let field_type = convert_type(&field.ty);
        if has_flatten_attr {
            types.push(field_type.ts_type);
        }
    }

    if types.is_empty() {
        return None;
    }

    Some(types.join(" & "))
}
