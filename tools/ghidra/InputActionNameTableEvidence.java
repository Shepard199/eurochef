import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
public class InputActionNameTableEvidence extends GhidraScript {
 public void run() throws Exception {
  Memory m=currentProgram.getMemory();
  long ptrBase=0x005e0a28L;
  long idBase=0x005e0a2cL;
  for(int i=0;i<27;i++){
    long p=Integer.toUnsignedLong(m.getInt(toAddr(ptrBase+i*8L)));
    int id=m.getInt(toAddr(idBase+i*8L));
    String s="<no-string>";
    try {
      Data d=getDataAt(toAddr(p));
      if(d!=null && d.hasStringValue()) s=d.getValue().toString();
      else {
        byte[] buf=new byte[128]; int n=m.getBytes(toAddr(p),buf);
        int z=0; while(z<n && buf[z]!=0)z++;
        s=new String(buf,0,z,java.nio.charset.StandardCharsets.US_ASCII);
      }
    } catch(Exception e){s="<err "+e.getMessage()+">";}
    println(String.format("%02d id=0x%02X ptr=0x%08X name=%s",i,id,p,s));
  }
 }
}