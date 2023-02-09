use std::{collections::BTreeMap, marker::PhantomData};

use utoipa::{
    openapi::{
        ContentBuilder, KnownFormat, ObjectBuilder, OneOfBuilder, Ref, RefOr, Response,
        ResponseBuilder, ResponsesBuilder, Schema, SchemaFormat, SchemaType,
    },
    IntoResponses, ToSchema,
};

use crate::{JsonResponse, Nothing};

/// A struct that can be used to combine 2 errors with the same status code
pub struct CombineErrors<E1, E2>(PhantomData<dyn Fn() -> (E1, E2)>);

impl<'__r, E1, E2> IntoResponses for CombineErrors<E1, E2>
where
    E1: ToSchema<'__r> + IntoResponses,
    E2: ToSchema<'__r> + IntoResponses,
{
    fn responses() -> BTreeMap<String, RefOr<Response>> {
        let status1 = match E1::responses().into_iter().next() {
            Some((status1, _)) => status1,
            _ => {
                panic!("Not supported")
            }
        };
        let status2 = match E2::responses().into_iter().next() {
            Some((status1, _)) => status1,
            _ => {
                panic!("Not supported")
            }
        };

        debug_assert_eq!(
            status1, status2,
            "CombineErrors can only be used for errors with same status"
        );

        let schema: RefOr<Schema> = OneOfBuilder::new()
            .item(Ref::from_schema_name(E1::schema().0))
            .item(Ref::from_schema_name(E2::schema().0))
            .into();

        ResponsesBuilder::new()
            .response(
                status1,
                ResponseBuilder::new()
                    .content("json", ContentBuilder::new().schema(schema).build()),
            )
            .build()
            .into()
    }
}

impl<'__r, T, M> ToSchema<'__r> for JsonResponse<T, M>
where
    T: ToSchema<'__r>,
    M: ToSchema<'__r>,
{
    fn schema() -> (&'__r str, RefOr<Schema>) {
        let null: RefOr<Schema> = ObjectBuilder::new()
            .schema_type(SchemaType::Object)
            .nullable(true)
            .default(Some(serde_json::Value::Null))
            .example(Some(serde_json::Value::Null))
            .build()
            .into();

        let obj = ObjectBuilder::new()
            .property(
                "status",
                ObjectBuilder::new()
                    .schema_type(SchemaType::Integer)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int32)))
                    .example(Some(200.into())),
            )
            .required("status");

        let obj = match T::schema() {
            ("", _) => obj.property("content", null.clone()),
            (name, _) => obj
                .property("content", RefOr::Ref(Ref::from_schema_name(name)))
                .required("content"),
        };

        let obj = match M::schema() {
            ("", _) => obj.property("meta", null.clone()),
            (name, _) => obj
                .property("meta", RefOr::Ref(Ref::from_schema_name(name)))
                .required("meta"),
        };

        ("JsonResponse", obj.into())
    }
}

impl ToSchema<'static> for Nothing {
    fn schema() -> (&'static str, RefOr<Schema>) {
        // A dummy implementation to make it recognizable by JsonResponse
        ("", ObjectBuilder::new().into())
    }
}
