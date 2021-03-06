use object::*;
use primitive::*;
use err::*;
use parser::Lexer;
use backend::Backend;
use file::File;


use std::io;
use std::ops::Deref;

/// General stream type. `T` is the info dictionary.
#[derive(Debug, Clone)]
pub struct Stream<T> {
    // General dictionary entries
    /// Filters that the `data` is currently encoded with (corresponds to both `/Filter` and
    /// `/DecodeParms` in the PDF specs), constructed in `from_primitive()`.
    filters: Vec<StreamFilter>,

    /// Eventual file containing the stream contentst
    file: Option<FileSpec>,
    /// Filters to apply to external file specified in `file`.
    file_filters: Vec<StreamFilter>,

    // TODO:
    /*
    /// Filters to apply to external file specified in `file`.
    #[pdf(key="FFilter")]
    file_filters: Vec<StreamFilter>,
    #[pdf(key="FDecodeParms")]
    file_decode_parms: Vec<DecodeParms>,
    /// Number of bytes in the decoded stream
    #[pdf(key="DL")]
    dl: Option<usize>,
    */
    // Specialized dictionary entries
    pub info: T,
    data: Vec<u8>,
}

impl<T: Default> Default for Stream<T> {
    fn default() -> Stream<T> {
        Stream {
            filters: Vec::new(),
            file: None,
            file_filters: Vec::new(),
            info: T::default(),
            data: Vec::new(),
        }
    }
}
impl<T> Stream<T> {
    /// If the stream is not encoded, this is a no-op. `decode()` should be called whenever it's uncertain
    /// whether the stream is encoded.
    pub fn decode(&mut self) -> Result<()> {
        for filter in &self.filters {
            eprintln!("Decode filter: {:?}", filter);
            self.data = decode(&self.data, filter)?;
        }
        self.filters.clear();
        Ok(())
    }
    pub fn encode(&mut self, _filter: StreamFilter) {
        // TODO this should add the filter to `self.filters` and encode the data with the given
        // filter
        unimplemented!();
    }
    pub fn get_length(&self) -> usize {
        self.data.len()
    }
    pub fn get_filters(&self) -> &[StreamFilter] {
        &self.filters
    }
    /// Get data - panics if it's not decoded in advance.
    /// Ideally I would have it take &mut self and do it itself,
    /// but that leads to problems in the code (aliasing)...
    pub fn get_data(&self) -> &[u8] {
        if self.get_filters().len() > 0 {
            panic!("Data not decoded! Consider using `get_data_raw`");
        }
        &self.data
    }
    /// Doesn't decode/unfilter the data.
    pub fn get_data_raw(&self) -> &[u8] {
        &self.data
    }
}
impl<T: Object> Object for Stream<T> {
    fn serialize<W: io::Write>(&self, _out: &mut W) -> io::Result<()> {
        unimplemented!();
    }
    fn from_primitive(p: Primitive, resolve: &Resolve) -> Result<Self> {
        // (TODO) there are a lot of `clone()` here because we can't consume the dict before we
        // pass it to T::from_primitive.
        let mut stream = PdfStream::from_primitive(p, resolve)?;
        let dict = &mut stream.info;

        let length = usize::from_primitive(
            dict.remove("Length").ok_or(Error::from(ErrorKind::EntryNotFound{key:"Length"}))?,
            resolve)?;
        assert_eq!(length, stream.data.len());

        let filters = Vec::<String>::from_primitive(
            dict.remove("Filter").ok_or(Error::from(ErrorKind::EntryNotFound{key:"Filter"}))?,
            resolve)?;

        let decode_params = Vec::<Dictionary>::from_primitive(
            dict.remove("DecodeParms").or(Some(Primitive::Null)).unwrap(),
            resolve)?;

        let file = Option::<FileSpec>::from_primitive(
            dict.remove("F").or(Some(Primitive::Null)).unwrap(),
            resolve)?;

        let file_filters = Vec::<String>::from_primitive(
            dict.remove("FFilter").or(Some(Primitive::Null)).unwrap(),
            resolve)?;

        let file_decode_params = Vec::<Dictionary>::from_primitive(
            dict.remove("FDecodeParms").or(Some(Primitive::Null)).unwrap(),
            resolve)?;


        let mut new_filters = Vec::new();
        let mut new_file_filters = Vec::new();

        for (i, filter) in filters.iter().enumerate() {
            let params = match decode_params.get(i) {
                Some(params) => params.clone(),
                None => Dictionary::default(),
            };
            new_filters.push(StreamFilter::from_kind_and_params(filter, params, resolve)?);
        }
        for (i, filter) in file_filters.iter().enumerate() {
            let params = match file_decode_params.get(i) {
                Some(params) => params.clone(),
                None => Dictionary::default(),
            };
            new_file_filters.push(StreamFilter::from_kind_and_params(filter, params, resolve)?);
        }

        Ok(Stream {
            // General
            filters: new_filters,
            file: file,
            file_filters: new_file_filters,
            // Special
            info: T::from_primitive(Primitive::Dictionary (dict.clone()), resolve)?,


            // Data
            data: stream.data,
        })
    }
}
impl<T> Deref for Stream<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.info
    }
}



#[derive(Object, Default)]
#[pdf(Type = "ObjStm")]
pub struct ObjStmInfo {

    /* TODO:  use Stream<T> .. but then I need the `offsets` here?
    #[pdf(key = "Filter")]
    pub filter: Vec<StreamFilter>,
    */

    // ObjStm fields
    #[pdf(key = "N")]
    /// Number of compressed objects in the stream.
    pub num_objects: i32,

    #[pdf(key = "First")]
    /// The byte offset in the decoded stream, of the first compressed object.
    pub first: i32,

    #[pdf(key = "Extends")]
    /// A reference to an eventual ObjectStream which this ObjectStream extends.
    pub extends: Option<i32>,

}

#[allow(dead_code)]
pub struct ObjectStream {
    stream: Stream<ObjStmInfo>,
    /// Byte offset of each object. Index is the object number.
    offsets:    Vec<usize>,
    /// The object number of this object.
    id:         ObjNr,
}
impl Object for ObjectStream {
    fn serialize<W: io::Write>(&self, out: &mut W) -> io::Result<()> {
        unimplemented!();
    }
    fn from_primitive(p: Primitive, resolve: &Resolve) -> Result<ObjectStream> {
        let mut stream = Stream::<ObjStmInfo>::from_primitive(p, resolve)?;
        stream.decode();

        let mut offsets = Vec::new();
        {
            let mut lexer = Lexer::new(&stream.get_data());
            for _ in 0..(stream.info.num_objects as ObjNr) {
                let _obj_nr = lexer.next()?.to::<ObjNr>()?;
                let offset = lexer.next()?.to::<usize>()?;
                offsets.push(offset);
            }
        }
        Ok(ObjectStream {
            stream: stream,
            offsets: offsets,
            id: 0, // TODO
        })
    }
}

impl ObjectStream {
    pub fn new<B: Backend>(file: &mut File<B>) -> ObjectStream {
        unimplemented!();
        /*
        let self_ref: PlainRef = (&file.promise::<ObjectStream>()).into();
        ObjectStream {
            stream: Stream::default(),
            offsets:    Vec::new(),
            id:         self_ref.id
        }
        */
    }
    pub fn id(&self) -> ObjNr {
        self.id
    }
    pub fn get_object_slice(&self, index: usize) -> Result<&[u8]> {
        if index >= self.offsets.len() {
            bail!(ErrorKind::ObjStmOutOfBounds {index: index, max: self.offsets.len()});
        }
        let start = self.stream.info.first as usize + self.offsets[index];
        let end = if index == self.offsets.len() - 1 {
            self.stream.data.len()
        } else {
            self.stream.info.first as usize + self.offsets[index + 1]
        };

        Ok(&self.stream.data[start..end])
    }
    /// Returns the number of contained objects
    pub fn n_objects(&self) -> usize {
        self.offsets.len()
    }
}
