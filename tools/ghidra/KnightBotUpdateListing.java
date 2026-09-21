import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Instruction;
public class KnightBotUpdateListing extends GhidraScript {
 @Override public void run() throws Exception {
   var a=toAddr(0x00465E30L); var end=toAddr(0x00465F90L);
   Instruction ins=getInstructionAt(a);
   while(ins!=null && ins.getAddress().compareTo(end)<0){
     println(ins.getAddress()+"  "+ins);
     ins=ins.getNext();
   }
 }
}