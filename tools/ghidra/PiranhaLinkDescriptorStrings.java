import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.DataType;
import ghidra.program.model.listing.Data;

public class PiranhaLinkDescriptorStrings extends GhidraScript {
    @Override
    public void run() throws Exception {
        long[] addrs={0x0061B07CL,0x0061B06CL,0x0061B054L,0x0061B044L,0x0061B02CL};
        for(long a:addrs){
            Data d=getDataAt(toAddr(a));
            println(String.format("0x%08X DATA=%s VALUE=%s",a,d==null?"<none>":d.getDataType().getName(),d==null?"<none>":String.valueOf(d.getValue())));
        }
    }
}